mod blueprints;
mod colonies;
mod extractions;
pub(crate) mod facility_owner;
mod jobs;
mod loaders;
mod planner;
mod planner_loaders;
mod planner_model;
mod planner_search;
pub(crate) mod rig_bonuses;
mod shell;
mod side_rail;
mod switcher;

use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, Utc};
use iced::{Element, Subscription, Task};

use self::planner::Planner;
pub use self::{
  loaders::{Activity, Blueprint, Colony, ColonyState, Extraction, IndustryJob, Loaded, Owner, RosterOwner},
  planner::{FacilityDefaults, Message as PlannerMessage},
  planner_loaders::{PlannerFacility, StaticCatalog, resolve_structure},
  planner_search::search_facilities,
};
use crate::{
  clients::{esi, eve_sso},
  store::{Database, images},
  ui::{
    components::resizable_pane::{self, PaneDrag},
    load_epoch::LoadEpoch,
  },
};

pub const EMPTY_INDUSTRY_SELECTION: i64 = 0;

pub const FACILITY_SEARCH_DEBOUNCE_MS: u64 = 200;

pub const FACILITY_SEARCH_MIN_CHARS: usize = planner::FACILITY_SEARCH_MIN_CHARS;

pub const RAIL_PANE_KEY: &str = "industry.jobs.rail";

const RAIL_PANE_DEFAULT_WIDTH: f32 = 280.0;
const RAIL_PANE_MIN_WIDTH: f32 = 240.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Filter {
  Active,
  #[default]
  All,
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
  Colonies,
  Extractions,
  #[default]
  Jobs,
  Planner,
}

impl Tab {
  pub const ALL: [Tab; 5] = [
    Tab::Jobs,
    Tab::Blueprints,
    Tab::Extractions,
    Tab::Colonies,
    Tab::Planner,
  ];

  pub fn from_id(id: &str) -> Option<Tab> {
    match id {
      "blueprints" => Some(Tab::Blueprints),
      "colonies" => Some(Tab::Colonies),
      "extractions" => Some(Tab::Extractions),
      "jobs" => Some(Tab::Jobs),
      "planner" => Some(Tab::Planner),
      _ => None,
    }
  }

  pub fn id(self) -> &'static str {
    match self {
      Tab::Blueprints => "blueprints",
      Tab::Colonies => "colonies",
      Tab::Extractions => "extractions",
      Tab::Jobs => "jobs",
      Tab::Planner => "planner",
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Tab::Blueprints => "Blueprints",
      Tab::Colonies => "Colonies",
      Tab::Extractions => "Extractions",
      Tab::Jobs => "Jobs",
      Tab::Planner => "Planner",
    }
  }

  pub(super) fn read_scopes(self) -> Vec<&'static str> {
    crate::features::shell::registry::sub_descriptor(self.sub_feature())
      .scopes
      .iter()
      .copied()
      .filter(|scope| !crate::clients::esi::scopes::is_write_scope(scope))
      .collect()
  }

  pub(super) fn sub_feature(self) -> crate::config::SubFeature {
    match self {
      Tab::Blueprints => crate::config::SubFeature::Blueprints,
      Tab::Colonies => crate::config::SubFeature::Colonies,
      Tab::Extractions => crate::config::SubFeature::Extractions,
      Tab::Jobs => crate::config::SubFeature::JobMonitoring,
      Tab::Planner => crate::config::SubFeature::Planner,
    }
  }
}

pub(super) fn enabled_tabs(flags: &crate::config::FeatureFlags) -> Vec<Tab> {
  Tab::ALL
    .into_iter()
    .filter(|tab| flags.is_sub_enabled(tab.sub_feature()))
    .collect()
}

pub(super) fn resolve_first_tab(enabled: &[Tab]) -> Tab {
  enabled.first().copied().unwrap_or_default()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BlueprintKind {
  #[default]
  All,
  Copies,
  Originals,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BlueprintSort {
  MaterialEfficiency,
  #[default]
  Name,
  Runs,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColonySort {
  #[default]
  Expiry,
  Tier,
  Value,
}

#[derive(Clone, Debug)]
pub enum Message {
  AssignPilotsChanged(bool),
  BlueprintKindSelected(BlueprintKind),
  BlueprintScrolled {
    absolute: f32,
  },
  BlueprintSearchChanged(String),
  BlueprintSortSelected(BlueprintSort),
  ColonySortSelected(ColonySort),
  FeaturesChanged(crate::config::FeatureFlags),
  FilterSelected(Filter),
  GroupBySelected(GroupBy),
  JobsScrolled {
    absolute: f32,
  },
  Loaded(Box<Loaded>),
  PaneSettled(&'static str, f32),
  PickerToggled,
  PilotsLoaded(Vec<planner_loaders::PlanPilot>),
  PlanBuild(i64),
  Planner(planner::Message),
  PlannerLoaded(Box<planner_loaders::PlannerData>),
  PlannerOnHandLoaded {
    epoch: u64,
    on_hand: HashMap<(i64, i64), i64>,
  },
  RailPaneDrag(f32),
  RailPaneDragEnd,
  RailPaneDragStart,
  ReauthRequested(i64),
  RequiredScopesChanged(Vec<&'static str>),
  ScopeSelected(Scope),
  TabSelected(Tab),
  Tick,
}

impl Message {
  pub fn loads_data(&self) -> bool {
    matches!(self, Message::Loaded(_))
  }
}

#[derive(Debug)]
pub struct State {
  active: Scope,
  assign_pilots: bool,
  blueprint_kind: BlueprintKind,
  blueprint_scroll_offset: f32,
  blueprint_search: String,
  blueprint_sort: BlueprintSort,
  blueprints: Vec<Blueprint>,
  clients: Option<Clients>,
  colonies: Vec<Colony>,
  colony_sort: ColonySort,
  enabled_tabs: Vec<Tab>,
  extractions: Vec<Extraction>,
  features: crate::config::FeatureFlags,
  filter: Filter,
  group_by: GroupBy,
  job_view: jobs::JobView,
  jobs: Vec<IndustryJob>,
  jobs_scroll_offset: f32,
  on_hand_epoch: LoadEpoch,
  picker_open: bool,
  planner: Planner,
  planner_catalog: Option<StaticCatalog>,
  rail_pane: PaneDrag,
  required_scopes: Vec<&'static str>,
  roster: Vec<RosterOwner>,
  tab: Tab,
}

impl State {
  pub fn new(
    active: i64,
    required_scopes: Vec<&'static str>,
    features: crate::config::FeatureFlags,
    facility_defaults: FacilityDefaults,
    planner_catalog: Option<StaticCatalog>,
    assign_pilots: bool,
  ) -> Self {
    let mut planner = Planner::new();
    planner.set_facility_defaults(facility_defaults);
    planner.set_assign_pilots(assign_pilots);
    let enabled_tabs = enabled_tabs(&features);
    State {
      active: if active == EMPTY_INDUSTRY_SELECTION {
        Scope::All
      } else {
        Scope::Char(active)
      },
      assign_pilots,
      blueprint_kind: BlueprintKind::default(),
      blueprint_scroll_offset: 0.0,
      blueprint_search: String::new(),
      blueprint_sort: BlueprintSort::default(),
      blueprints: Vec::new(),
      clients: None,
      colonies: Vec::new(),
      colony_sort: ColonySort::default(),
      enabled_tabs: enabled_tabs.clone(),
      extractions: Vec::new(),
      features,
      filter: Filter::default(),
      group_by: GroupBy::default(),
      job_view: jobs::JobView::default(),
      jobs: Vec::new(),
      jobs_scroll_offset: 0.0,
      on_hand_epoch: LoadEpoch::default(),
      picker_open: false,
      planner,
      planner_catalog,
      rail_pane: PaneDrag::with_min_width(
        RAIL_PANE_DEFAULT_WIDTH,
        RAIL_PANE_MIN_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      )
      .right_anchored(true),
      required_scopes,
      roster: Vec::new(),
      tab: resolve_first_tab(&enabled_tabs),
    }
  }

  pub fn active(&self) -> Scope {
    self.active
  }

  pub fn active_tab(&self) -> Tab {
    self.tab
  }

  pub fn select_tab_by_id(&mut self, id: &str) -> bool {
    match Tab::from_id(id) {
      Some(tab) => {
        self.tab = tab;
        true
      }
      None => false,
    }
  }

  pub fn set_clients(&mut self, esi: std::sync::Arc<esi::Client>, sso: std::sync::Arc<eve_sso::Client>) {
    self.clients = Some(Clients {
      esi,
      sso,
    });
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.planner.set_pane_host_width(host_width);
    self.rail_pane.set_host_width(host_width);
  }

  pub(super) fn set_required_scopes(&mut self, scopes: Vec<&'static str>) {
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

  pub fn with_restored_panes(mut self, ui: &crate::features::shell::window_state::UiState) -> Self {
    self.planner = self.planner.with_restored_panes(ui);
    let host_width = ui.host_width("main", crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.rail_pane = PaneDrag::from_store_with_min(
      ui,
      RAIL_PANE_KEY,
      RAIL_PANE_DEFAULT_WIDTH,
      RAIL_PANE_MIN_WIDTH,
      host_width,
    )
    .right_anchored(true);
    self
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

  pub(super) fn colony_sort(&self) -> ColonySort {
    self.colony_sort
  }

  pub(crate) fn facility_search_target(&self) -> Option<(i64, u64)> {
    self
      .planner
      .facility_picker()
      .map(|state| (state.type_id, state.search_generation))
  }

  pub(super) fn visible_blueprints(&self) -> Vec<&Blueprint> {
    self
      .blueprints
      .iter()
      .filter(|blueprint| self.is_authorized(blueprint.owner))
      .collect()
  }

  pub(super) fn visible_colonies(&self) -> Vec<&Colony> {
    self
      .colonies
      .iter()
      .filter(|colony| self.colony_in_scope(colony))
      .collect()
  }

  fn colony_in_scope(&self, colony: &Colony) -> bool {
    match self.active {
      Scope::All => true,
      Scope::Char(id) => colony.character_id == id,
      Scope::Corp(id) => self
        .roster
        .iter()
        .any(|owner| !owner.is_corporation && owner.id == colony.character_id && owner.corporation_id == Some(id)),
    }
  }

  pub(super) fn visible_extractions(&self) -> Vec<&Extraction> {
    let corporation = match self.active {
      Scope::All => None,
      Scope::Corp(id) => Some(id),
      Scope::Char(id) => self
        .roster
        .iter()
        .find(|owner| owner.id == id && !owner.is_corporation)
        .and_then(|owner| owner.corporation_id),
    };
    self
      .extractions
      .iter()
      .filter(|extraction| corporation.is_none_or(|id| extraction.corporation_id == id))
      .collect()
  }

  pub(super) fn filter(&self) -> Filter {
    self.filter
  }

  pub(super) fn group_by(&self) -> GroupBy {
    self.group_by
  }

  pub(in crate::features::industry) fn job_view(&self) -> &jobs::JobView {
    &self.job_view
  }

  pub(super) fn jobs(&self) -> &[IndustryJob] {
    &self.jobs
  }

  pub(super) fn jobs_scroll_offset(&self) -> f32 {
    self.jobs_scroll_offset
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

  fn pilot_identities(&self) -> Vec<(i64, String, Option<std::path::PathBuf>)> {
    self
      .roster
      .iter()
      .filter(|owner| !owner.is_corporation)
      .filter(|owner| match self.active {
        Scope::All => true,
        Scope::Char(id) => owner.id == id,
        Scope::Corp(id) => owner.corporation_id == Some(id),
      })
      .map(|owner| {
        let portrait = owner.portrait.as_ref().and_then(images::ImageState::path);
        (owner.id, owner.name.clone(), portrait)
      })
      .collect()
  }

  pub(super) fn planner(&self) -> &Planner {
    &self.planner
  }

  pub(crate) fn planner_catalog(&self) -> Option<&StaticCatalog> {
    self.planner_catalog.as_ref()
  }

  pub(super) fn rail_pane_width(&self) -> f32 {
    self.rail_pane.width()
  }

  fn is_dragging_rail(&self) -> bool {
    self.rail_pane.is_active()
  }

  pub(super) fn required_scopes(&self) -> &[&'static str] {
    &self.required_scopes
  }

  pub(super) fn roster(&self) -> &[RosterOwner] {
    &self.roster
  }

  pub(super) fn enabled_tabs(&self) -> &[Tab] {
    &self.enabled_tabs
  }

  pub(super) fn sync_features(&mut self, features: crate::config::FeatureFlags) {
    self.features = features;
    self.enabled_tabs = enabled_tabs(&features);
    if !self.enabled_tabs.contains(&self.tab) {
      self.tab = resolve_first_tab(&self.enabled_tabs);
    }
  }

  pub(super) fn tab_scope_gate(&self) -> Option<(i64, &str, Vec<&'static str>)> {
    let Scope::Char(id) = self.active else {
      return None;
    };
    let owner = self
      .roster
      .iter()
      .find(|owner| owner.id == id && !owner.is_corporation)?;
    let missing =
      crate::ui::components::forbidden::missing_scopes(owner.granted_scopes.as_deref(), &self.tab.read_scopes());
    if missing.is_empty() {
      return None;
    }
    Some((id, owner.name.as_str(), missing))
  }

  pub(super) fn tab(&self) -> Tab {
    self.tab
  }

  #[cfg(test)]
  pub(super) fn seed_colonies(&mut self, colonies: Vec<Colony>) {
    self.colonies = colonies;
  }

  #[cfg(test)]
  pub(super) fn seed_extractions(&mut self, extractions: Vec<Extraction>) {
    self.extractions = extractions;
  }

  #[cfg(test)]
  pub(super) fn seed_tab(&mut self, tab: Tab) {
    self.tab = tab;
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

  fn rebuild_job_view(&mut self, now: DateTime<Utc>) {
    let visible: Vec<usize> = self
      .jobs
      .iter()
      .enumerate()
      .filter(|(_, job)| self.is_authorized(job.owner))
      .map(|(index, _)| index)
      .collect();
    self.job_view = jobs::JobView::build(&self.jobs, &visible, self.filter, self.group_by, now);
  }
}

#[derive(Clone)]
pub(crate) struct Clients {
  pub(crate) esi: std::sync::Arc<esi::Client>,
  pub(crate) sso: std::sync::Arc<eve_sso::Client>,
}

impl std::fmt::Debug for Clients {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Clients").finish_non_exhaustive()
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
    async move {
      let tree = crate::store::repo::industry::load_plan(&db, id).await.ok().flatten()?;
      let segments = crate::store::repo::industry::segments_for_plan(&db, id)
        .await
        .unwrap_or_default();
      Some((tree, segments))
    },
    |loaded| match loaded {
      Some((tree, segments)) => Message::Planner(planner::Message::PlanRestored {
        segments,
        tree: Box::new(tree),
      }),
      None => Message::Tick,
    },
  )
}

fn load_planner(state: &State, db: &Database, scope: Scope, catalog: Option<StaticCatalog>) -> Task<Message> {
  let esi = state
    .clients
    .as_ref()
    .map(|clients| std::sync::Arc::clone(&clients.esi));
  let sso = state
    .clients
    .as_ref()
    .map(|clients| std::sync::Arc::clone(&clients.sso));
  planner::load(db.clone(), scope, catalog, esi, sso).map(|data| Message::PlannerLoaded(Box::new(data)))
}

fn handle_planner(state: &mut State, db: &Database, message: planner::Message) -> Task<Message> {
  match message {
    planner::Message::ShoppingListCopied => {
      state.planner.update(message);
      iced::clipboard::write(state.planner.shopping_list())
    }
    planner::Message::PlanSaveRequested => match (state.planner.snapshot(), state.planner.save_name()) {
      (Some(tree), Some(name)) => save_plan(db, name, tree, state.planner.segments()),
      _ => Task::none(),
    },
    planner::Message::PlanLoadRequested(id) => load_plan(db, id),
    planner::Message::PlanDeleteRequested(id) => delete_plan(db, id),
    planner::Message::PaneDragEnd => {
      state.planner.update(message);
      Task::done(Message::PaneSettled(
        planner::DETAIL_PANE_KEY,
        state.planner.detail_pane_ratio(),
      ))
    }
    other => {
      let before = state.planner.build_sites();
      state.planner.update(other);
      let after = state.planner.build_sites();
      if before == after {
        Task::none()
      } else {
        let epoch = state.on_hand_epoch.next();
        load_on_hand(db, after, epoch)
      }
    }
  }
}

fn load_pilots(db: &Database, identities: Vec<(i64, String, Option<std::path::PathBuf>)>) -> Task<Message> {
  if identities.is_empty() {
    return Task::done(Message::PilotsLoaded(Vec::new()));
  }
  let db = db.clone();
  Task::perform(
    async move { planner_loaders::plan_pilots(&db, &identities).await },
    Message::PilotsLoaded,
  )
}

fn load_on_hand(db: &Database, sites: Vec<i64>, epoch: u64) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      crate::store::repo::assets::on_hand_at_build_sites(&db, &sites)
        .await
        .unwrap_or_default()
    },
    move |on_hand| Message::PlannerOnHandLoaded {
      epoch,
      on_hand,
    },
  )
}

pub fn facility_search(
  db: &Database,
  esi: std::sync::Arc<esi::Client>,
  sso: std::sync::Arc<eve_sso::Client>,
  type_id: i64,
  query: String,
  generation: u64,
) -> Task<Message> {
  if query.trim().chars().count() < planner::FACILITY_SEARCH_MIN_CHARS {
    return Task::none();
  }
  let db = db.clone();
  Task::perform(
    async move {
      tokio::time::sleep(Duration::from_millis(FACILITY_SEARCH_DEBOUNCE_MS)).await;
      planner_search::search_facilities(db, esi, sso, query).await
    },
    move |results| {
      Message::Planner(PlannerMessage::FacilitySearchResults {
        generation,
        results,
        type_id,
      })
    },
  )
}

fn save_plan(
  db: &Database,
  name: String,
  tree: crate::store::model::PlanTree,
  segments: Vec<crate::store::model::PlanSegment>,
) -> Task<Message> {
  let db = db.clone();
  Task::perform(
    async move {
      if let Ok(plan) = crate::store::repo::industry::create_plan(&db, &name, &tree).await {
        let _ = crate::store::repo::industry::replace_plan_segments(&db, plan.id(), &segments).await;
      }
      list_saved_plans(&db).await
    },
    |plans| Message::Planner(planner::Message::PlansListed(plans)),
  )
}

pub fn subscription(state: &State) -> Subscription<Message> {
  let tick = iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick);
  let drag = if state.planner.is_dragging_pane() {
    iced::event::listen_with(|event, _status, _id| {
      resizable_pane::drag_event(
        event,
        |x| Message::Planner(planner::Message::PaneDrag(x)),
        Message::Planner(planner::Message::PaneDragEnd),
      )
    })
  } else if state.is_dragging_rail() {
    iced::event::listen_with(|event, _status, _id| {
      resizable_pane::drag_event(event, Message::RailPaneDrag, Message::RailPaneDragEnd)
    })
  } else {
    return tick;
  };
  Subscription::batch([tick, drag])
}

fn handle_plan_build(state: &mut State, db: &Database, blueprint_type_id: i64) -> Task<Message> {
  state.tab = Tab::Planner;
  let mut tasks = vec![list_plans(db)];
  if state.planner.is_loaded() {
    state.planner.seed_from_blueprint(blueprint_type_id);
  } else {
    state.planner.queue_blueprint_seed(blueprint_type_id);
    tasks.push(load_planner(state, db, state.active, state.planner_catalog.clone()));
  }
  Task::batch(tasks)
}

pub fn update(state: &mut State, message: Message, db: &Database, now: DateTime<Utc>) -> Task<Message> {
  match message {
    Message::BlueprintKindSelected(..)
    | Message::BlueprintScrolled {
      ..
    }
    | Message::BlueprintSearchChanged(..)
    | Message::BlueprintSortSelected(..) => update_blueprints(state, message),
    Message::ColonySortSelected(sort) => {
      state.colony_sort = sort;
      Task::none()
    }
    Message::FilterSelected(..)
    | Message::GroupBySelected(..)
    | Message::JobsScrolled {
      ..
    }
    | Message::Tick => update_jobs(state, message, now),
    Message::RailPaneDrag(..) | Message::RailPaneDragEnd | Message::RailPaneDragStart => {
      update_rail_pane(state, message)
    }
    Message::AssignPilotsChanged(..)
    | Message::PilotsLoaded(..)
    | Message::PlanBuild(..)
    | Message::Planner(..)
    | Message::PlannerLoaded(..)
    | Message::PlannerOnHandLoaded {
      ..
    } => update_planner_messages(state, message, db),
    Message::FeaturesChanged(features) => handle_features_changed(state, features, db, now),
    Message::Loaded(loaded) => handle_loaded(state, *loaded, db, now),
    Message::PickerToggled => {
      state.picker_open = !state.picker_open;
      Task::none()
    }
    Message::PaneSettled(..) => Task::none(),
    Message::ReauthRequested(_) => Task::none(),
    Message::RequiredScopesChanged(scopes) => {
      state.set_required_scopes(scopes);
      Task::none()
    }
    Message::ScopeSelected(scope) => handle_scope_selected(state, scope, db, now),
    Message::TabSelected(tab) => handle_tab_selected(state, tab, db),
  }
}

fn handle_features_changed(
  state: &mut State,
  features: crate::config::FeatureFlags,
  db: &Database,
  now: DateTime<Utc>,
) -> Task<Message> {
  let prev = state.tab;
  state.sync_features(features);
  if state.tab == prev {
    return Task::none();
  }
  update(state, Message::TabSelected(state.tab), db, now)
}

fn handle_loaded(state: &mut State, loaded: Loaded, db: &Database, now: DateTime<Utc>) -> Task<Message> {
  let Loaded {
    blueprints,
    colonies,
    extractions,
    facility_defaults,
    jobs,
    roster,
    scope,
  } = loaded;
  if scope != state.active {
    return Task::none();
  }
  state.planner.set_facility_defaults(facility_defaults);
  state.blueprints = blueprints;
  state.colonies = colonies;
  state.extractions = extractions;
  state.jobs = jobs;
  state.roster = roster;
  state.rebuild_job_view(now);
  if state.assign_pilots && state.tab == Tab::Planner {
    return load_pilots(db, state.pilot_identities());
  }
  Task::none()
}

fn handle_scope_selected(state: &mut State, scope: Scope, db: &Database, now: DateTime<Utc>) -> Task<Message> {
  state.active = scope;
  state.picker_open = false;
  state.blueprint_scroll_offset = 0.0;
  state.jobs_scroll_offset = 0.0;
  state.rebuild_job_view(now);
  let mut tasks = vec![reload(db, scope, &state.required_scopes)];
  if state.planner.is_loaded() {
    tasks.push(load_planner(state, db, scope, state.planner_catalog.clone()));
  }
  Task::batch(tasks)
}

fn handle_tab_selected(state: &mut State, tab: Tab, db: &Database) -> Task<Message> {
  state.tab = tab;
  if tab != Tab::Planner {
    return Task::none();
  }
  let mut tasks = vec![list_plans(db)];
  if !state.planner.is_loaded() {
    tasks.push(load_planner(state, db, state.active, state.planner_catalog.clone()));
  }
  if state.assign_pilots {
    tasks.push(load_pilots(db, state.pilot_identities()));
  }
  Task::batch(tasks)
}

fn update_blueprints(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::BlueprintKindSelected(kind) => {
      state.blueprint_kind = kind;
      state.blueprint_scroll_offset = 0.0;
    }
    Message::BlueprintScrolled {
      absolute,
    } => {
      state.blueprint_scroll_offset = absolute;
    }
    Message::BlueprintSearchChanged(query) => {
      state.blueprint_search = query;
      state.blueprint_scroll_offset = 0.0;
    }
    Message::BlueprintSortSelected(sort) => {
      state.blueprint_sort = sort;
      state.blueprint_scroll_offset = 0.0;
    }
    _ => unreachable!("update_blueprints only handles blueprint-tab messages"),
  }
  Task::none()
}

fn update_jobs(state: &mut State, message: Message, now: DateTime<Utc>) -> Task<Message> {
  match message {
    Message::FilterSelected(filter) => {
      state.filter = filter;
      state.jobs_scroll_offset = 0.0;
      state.rebuild_job_view(now);
    }
    Message::GroupBySelected(group_by) => {
      state.group_by = group_by;
      state.jobs_scroll_offset = 0.0;
      state.rebuild_job_view(now);
    }
    Message::JobsScrolled {
      absolute,
    } => {
      state.jobs_scroll_offset = absolute;
    }
    Message::Tick => {
      state.rebuild_job_view(now);
    }
    _ => unreachable!("update_jobs only handles jobs-tab messages"),
  }
  Task::none()
}

fn update_rail_pane(state: &mut State, message: Message) -> Task<Message> {
  match message {
    Message::RailPaneDrag(x) => {
      state.rail_pane.drag_to(x);
      Task::none()
    }
    Message::RailPaneDragEnd => {
      state.rail_pane.end();
      Task::done(Message::PaneSettled(RAIL_PANE_KEY, state.rail_pane.ratio()))
    }
    Message::RailPaneDragStart => {
      state.rail_pane.start();
      Task::none()
    }
    _ => unreachable!("update_rail_pane only handles rail-pane messages"),
  }
}

fn update_planner_messages(state: &mut State, message: Message, db: &Database) -> Task<Message> {
  match message {
    Message::AssignPilotsChanged(enabled) => {
      state.assign_pilots = enabled;
      state.planner.set_assign_pilots(enabled);
      if enabled && state.tab == Tab::Planner {
        load_pilots(db, state.pilot_identities())
      } else {
        Task::none()
      }
    }
    Message::PilotsLoaded(pilots) => {
      state.planner.set_pilots(pilots);
      Task::none()
    }
    Message::PlanBuild(blueprint_type_id) => handle_plan_build(state, db, blueprint_type_id),
    Message::Planner(planner_message) => handle_planner(state, db, planner_message),
    Message::PlannerLoaded(data) => {
      if state.planner_catalog.is_none() {
        state.planner_catalog = Some(StaticCatalog::from_planner_data(&data));
      }
      let facility_intel = data.facility_intel.clone();
      let rig_catalog = data.rig_catalog.clone();
      state.planner.apply_data(*data);
      state.planner.set_rig_data(facility_intel, rig_catalog);
      let epoch = state.on_hand_epoch.next();
      load_on_hand(db, state.planner.build_sites(), epoch)
    }
    Message::PlannerOnHandLoaded {
      epoch,
      on_hand,
    } => {
      if !state.on_hand_epoch.matches(epoch) {
        return Task::none();
      }
      state.planner.set_on_hand(on_hand);
      Task::none()
    }
    _ => unreachable!("update_planner_messages only handles planner-tab messages"),
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

  #[test]
  fn tab_id_round_trips_for_every_variant() {
    for tab in Tab::ALL {
      assert_eq!(Tab::from_id(tab.id()), Some(tab));
      assert!(!tab.label().is_empty());
      let _ = tab.sub_feature();
    }
    assert_eq!(Tab::from_id("nope"), None);
  }

  fn character_owner(id: i64, scopes: Option<&str>) -> RosterOwner {
    RosterOwner {
      corp: "TST".to_owned(),
      corporation_id: Some(98),
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
      slots: SlotCaps::default(),
    }
  }

  fn job(owner: Owner, job_id: i64, activity: Activity, end: &str) -> IndustryJob {
    IndustryJob {
      activity,
      blueprint_icon: crate::store::images::IconResolution::Missing,
      cost: 1_000.0,
      end_date: end.to_owned(),
      facility: "Jita IV - Moon 4".to_owned(),
      installer: "Pilot 1".to_owned(),
      installer_id: owner.id(),
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
    let mut state = State::new(
      EMPTY_INDUSTRY_SELECTION,
      required(),
      crate::config::FeatureFlags::default(),
      FacilityDefaults::default(),
      None,
      false,
    );
    state.active = active;
    state.roster = roster;
    state.jobs = jobs;
    state.rebuild_job_view(now());
    state
  }

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
    state.extractions = vec![
      extraction(
        98,
        1,
        "2026-06-12T00:00:00Z",
        "2026-06-14T00:00:00Z",
        "2026-06-16T00:00:00Z",
      ),
      extraction(
        98,
        2,
        "2026-06-13T00:00:00Z",
        "2026-06-13T18:00:00Z",
        "2026-06-15T00:00:00Z",
      ),
    ];
    state.blueprints = vec![
      blueprint(Owner::Character(1), 1, "Rifter Blueprint", -1, 10, 20, false),
      blueprint(Owner::Character(1), 2, "Hobgoblin I Blueprint", 12, 4, 8, false),
      blueprint(Owner::Character(2), 3, "Hidden Blueprint", 5, 0, 0, false),
      blueprint(Owner::Corporation(98), 4, "Sulfuric Acid Reaction", -1, 0, 0, true),
    ];
    state
  }

  fn extraction(corporation_id: i64, moon_id: i64, start: &str, arrival: &str, decay: &str) -> Extraction {
    Extraction {
      chunk_arrival_time: Some(arrival.to_owned()),
      corporation_id,
      extraction_start_time: Some(start.to_owned()),
      moon_id,
      moon_name: Some(format!("Moon {moon_id}")),
      natural_decay_time: Some(decay.to_owned()),
      security: Some(0.5),
      structure: "Athanor".to_owned(),
      system_name: Some("Tama".to_owned()),
    }
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
      type_icon: crate::store::images::IconResolution::Missing,
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

      assert_eq!(visible.len(), 3);
      assert!(visible.iter().all(|bp| bp.owner != Owner::Character(2)));
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
  }

  mod pilot_identities {
    use pretty_assertions::assert_eq;

    use super::*;

    fn roster() -> Vec<RosterOwner> {
      vec![
        character_owner(1, None),
        character_owner(2, None),
        corporation_owner(98),
      ]
    }

    #[test]
    fn it_keeps_only_characters_under_the_all_scope() {
      let state = state_with(Scope::All, roster(), Vec::new());

      let ids: Vec<i64> = state.pilot_identities().into_iter().map(|(id, ..)| id).collect();

      assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn it_keeps_the_single_pilot_under_a_character_scope() {
      let state = state_with(Scope::Char(2), roster(), Vec::new());

      let ids: Vec<i64> = state.pilot_identities().into_iter().map(|(id, ..)| id).collect();

      assert_eq!(ids, vec![2]);
    }

    #[test]
    fn it_keeps_the_corporations_members_under_a_corp_scope() {
      let state = state_with(Scope::Corp(98), roster(), Vec::new());

      let ids: Vec<i64> = state.pilot_identities().into_iter().map(|(id, ..)| id).collect();

      assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn it_is_empty_for_a_corp_scope_with_no_members() {
      let state = state_with(Scope::Corp(999), roster(), Vec::new());

      assert!(state.pilot_identities().is_empty());
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
      let _ = update(&mut state, Message::TabSelected(Tab::Extractions), &db, n);
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
      let on_hand_epoch = state.on_hand_epoch.current();
      let _ = update(
        &mut state,
        Message::PlannerOnHandLoaded {
          epoch: on_hand_epoch,
          on_hand: HashMap::new(),
        },
        &db,
        n,
      );
      let _ = update(&mut state, Message::PlanBuild(681), &db, n);
      let _ = update(&mut state, Message::Planner(planner::Message::BreakDownAll), &db, n);
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
        Message::Planner(planner::Message::NodeBrokenDown {
          type_id: 34,
        }),
        &db,
        n,
      );
      let _ = update(&mut state, Message::Tick, &db, n);
      let _ = update(&mut state, Message::RailPaneDragStart, &db, n);
      let _ = update(&mut state, Message::RailPaneDrag(640.0), &db, n);
      let _ = update(&mut state, Message::RailPaneDragEnd, &db, n);
      let _ = update(&mut state, Message::PickerToggled, &db, n);
      let _ = update(&mut state, Message::PilotsLoaded(Vec::new()), &db, n);
      let _ = update(&mut state, Message::AssignPilotsChanged(true), &db, n);
      let _ = update(&mut state, Message::AssignPilotsChanged(false), &db, n);
      let _ = update(&mut state, Message::ReauthRequested(1), &db, n);
      let _ = update(&mut state, Message::ScopeSelected(Scope::Char(1)), &db, n);

      let fresh = Loaded {
        blueprints: Vec::new(),
        colonies: Vec::new(),
        extractions: Vec::new(),
        facility_defaults: FacilityDefaults::default(),
        jobs: Vec::new(),
        roster: Vec::new(),
        scope: state.active,
      };
      let _ = update(&mut state, Message::Loaded(Box::new(fresh)), &db, n);
      let stale = Loaded {
        blueprints: Vec::new(),
        colonies: Vec::new(),
        extractions: Vec::new(),
        facility_defaults: FacilityDefaults::default(),
        jobs: Vec::new(),
        roster: Vec::new(),
        scope: Scope::Char(424_242),
      };
      let _ = update(&mut state, Message::Loaded(Box::new(stale)), &db, n);
    }

    #[tokio::test]
    async fn it_drops_on_hand_for_a_build_site_the_planner_switched_away_from() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = populated();
      let n = now();

      let site_a = 60_003_760;
      let site_b = 60_008_494;
      let type_id = 34;

      let stale_epoch = state.on_hand_epoch.current();

      let fresh_epoch = state.on_hand_epoch.next();
      let _ = update(
        &mut state,
        Message::PlannerOnHandLoaded {
          epoch: fresh_epoch,
          on_hand: HashMap::from([((site_b, type_id), 5)]),
        },
        &db,
        n,
      );

      let _ = update(
        &mut state,
        Message::PlannerOnHandLoaded {
          epoch: stale_epoch,
          on_hand: HashMap::from([((site_a, type_id), 99)]),
        },
        &db,
        n,
      );

      assert_eq!(
        state.planner.on_hand_at(site_b, type_id),
        5,
        "the current site's on-hand stock is preserved"
      );
      assert_eq!(
        state.planner.on_hand_at(site_a, type_id),
        0,
        "on-hand for the abandoned site must not overwrite the current map"
      );
    }
  }

  mod enum_defaults {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_preserves_the_segmented_control_defaults_after_alphabetizing() {
      assert_eq!(Filter::default(), Filter::All);
      assert_eq!(BlueprintKind::default(), BlueprintKind::All);
      assert_eq!(BlueprintSort::default(), BlueprintSort::Name);
    }
  }

  mod job_view {
    use pretty_assertions::assert_eq;

    use super::{super::jobs::JobRowItem, *};

    fn job_ids(state: &State) -> Vec<i64> {
      state
        .job_view()
        .rows
        .iter()
        .filter_map(|row| match row {
          JobRowItem::Job(index) => Some(state.jobs()[*index].job_id),
          JobRowItem::Header(_) => None,
        })
        .collect()
    }

    #[test]
    fn it_drops_unauthorized_characters_from_the_memoized_set() {
      let state = populated();

      assert!(
        state
          .job_view()
          .rows
          .iter()
          .all(|row| !matches!(row, JobRowItem::Job(index) if state.jobs()[*index].owner == Owner::Character(2)))
      );
    }

    #[tokio::test]
    async fn it_emits_a_header_row_per_group() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = populated();

      let _ = update(&mut state, Message::GroupBySelected(GroupBy::Owner), &db, now());

      let headers = state
        .job_view()
        .rows
        .iter()
        .filter(|row| matches!(row, JobRowItem::Header(_)))
        .count();
      let jobs = state
        .job_view()
        .rows
        .iter()
        .filter(|row| matches!(row, JobRowItem::Job(_)))
        .count();

      assert!(headers >= 1);
      assert_eq!(jobs, 3);
    }

    #[tokio::test]
    async fn it_filters_to_ready_jobs_only() {
      let db = crate::store::open_test().await.unwrap();
      let mut state = populated();

      let _ = update(&mut state, Message::FilterSelected(Filter::Ready), &db, now());

      assert_eq!(job_ids(&state), vec![11]);
    }

    #[test]
    fn it_orders_ready_jobs_first_then_by_end_time() {
      let state = populated();

      assert_eq!(job_ids(&state), vec![11, 10, 13]);
      assert_eq!(state.job_view().counts.total, 3);
      assert_eq!(state.job_view().counts.ready, 1);
      assert_eq!(state.job_view().counts.active, 2);
    }
  }

  mod list_saved_plans {
    use pretty_assertions::assert_eq;

    use crate::store::{
      model::{PlanTree, PlanType},
      repo::industry::{self as industry_repo},
    };

    fn sample_tree() -> PlanTree {
      PlanTree {
        product_type_id: 22_544,
        root_facility_system: None,
        runs: 1,
        types: vec![PlanType {
          built: false,
          facility_structure: None,
          facility_system: None,
          me: 10,
          te: 20,
          type_id: 22_544,
          use_stock: false,
        }],
      }
    }

    #[tokio::test]
    async fn it_drops_a_plan_once_deleted() {
      let db = crate::store::open_test().await.unwrap();
      let plan = industry_repo::create_plan(&db, "Doomed", &sample_tree()).await.unwrap();

      industry_repo::delete_plan(&db, plan.id()).await.unwrap();

      assert!(super::list_saved_plans(&db).await.is_empty());
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
  }

  mod rendering {
    use super::*;

    #[test]
    fn it_renders_an_empty_blueprints_tab() {
      let mut state = state_with(Scope::All, Vec::new(), Vec::new());
      state.tab = Tab::Blueprints;

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_an_empty_extractions_tab() {
      let mut state = state_with(Scope::All, Vec::new(), Vec::new());
      state.tab = Tab::Extractions;

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_an_empty_state() {
      let state = state_with(Scope::All, Vec::new(), Vec::new());

      let _el: Element<'_, Message> = view(&state, &required(), now());
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
    fn it_renders_each_group_by() {
      let mut state = populated();
      for group_by in [GroupBy::None, GroupBy::Owner, GroupBy::Activity, GroupBy::Facility] {
        state.group_by = group_by;
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
    fn it_renders_the_combined_view() {
      let state = populated();

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_the_extractions_tab() {
      let mut state = populated();
      state.tab = Tab::Extractions;

      let _el: Element<'_, Message> = view(&state, &required(), now());
    }

    #[test]
    fn it_renders_the_forbidden_gate_for_an_unauthorized_pilot() {
      let mut state = populated();
      state.active = Scope::Char(2);

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
      state.planner.update(planner::Message::ProductPicked(22_544));

      {
        let _loaded: Element<'_, Message> = view(&state, &required(), now());
      }
      state.planner.update(planner::Message::NodeBrokenDown {
        type_id: 17_478,
      });
      state
        .planner
        .update(planner::Message::RightTabSelected(planner::RightTab::Plans));
      {
        let _empty_plans: Element<'_, Message> = view(&state, &required(), now());
      }
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
        type_id: 34,
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
  }

  mod enabled_tabs {
    use pretty_assertions::assert_eq;

    use super::*;

    fn only(sub: crate::config::SubFeature) -> crate::config::FeatureFlags {
      let mut flags = crate::config::FeatureFlags::default();
      for candidate in crate::config::SubFeature::ALL {
        flags.set_sub_enabled(candidate, candidate == sub);
      }
      flags
    }

    #[test]
    fn it_keeps_strip_order_with_every_sub_feature_enabled() {
      let tabs = enabled_tabs(&crate::config::FeatureFlags::default());

      assert_eq!(
        tabs,
        vec![
          Tab::Jobs,
          Tab::Blueprints,
          Tab::Extractions,
          Tab::Colonies,
          Tab::Planner
        ]
      );
    }

    #[test]
    fn it_drops_a_disabled_sub_feature_from_the_strip() {
      let tabs = enabled_tabs(&only(crate::config::SubFeature::Planner));

      assert_eq!(tabs, vec![Tab::Planner]);
    }
  }

  mod sync_features {
    use pretty_assertions::assert_eq;

    use super::*;

    fn without(sub: crate::config::SubFeature) -> crate::config::FeatureFlags {
      let mut flags = crate::config::FeatureFlags::default();
      flags.set_sub_enabled(sub, false);
      flags
    }

    #[test]
    fn it_redirects_off_a_disabled_active_tab_to_the_first_enabled_tab() {
      let mut state = state_with(Scope::All, Vec::new(), Vec::new());
      state.seed_tab(Tab::Jobs);

      state.sync_features(without(crate::config::SubFeature::JobMonitoring));

      assert_eq!(state.tab(), Tab::Blueprints);
    }

    #[test]
    fn it_keeps_a_still_enabled_active_tab() {
      let mut state = state_with(Scope::All, Vec::new(), Vec::new());
      state.seed_tab(Tab::Planner);

      state.sync_features(without(crate::config::SubFeature::JobMonitoring));

      assert_eq!(state.tab(), Tab::Planner);
    }
  }

  mod tab_scope_gate {
    use super::*;

    #[test]
    fn it_does_not_gate_an_authorized_char_scope() {
      let granted = granted();
      let state = state_with(Scope::Char(1), vec![character_owner(1, Some(&granted))], Vec::new());

      assert!(state.tab_scope_gate().is_none());
    }

    #[test]
    fn it_gates_a_char_scope_missing_the_active_tab_scope() {
      let state = state_with(Scope::Char(1), vec![character_owner(1, None)], Vec::new());

      assert!(state.tab_scope_gate().is_some());
    }

    #[test]
    fn it_never_gates_the_combined_scope() {
      let state = state_with(Scope::All, vec![character_owner(1, None)], Vec::new());

      assert!(state.tab_scope_gate().is_none());
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

  mod visible_extractions {
    use pretty_assertions::assert_eq;

    use super::*;

    fn roster_and_extractions() -> (Vec<RosterOwner>, Vec<Extraction>) {
      let granted = granted();
      let roster = vec![character_owner(1, Some(&granted)), corporation_owner(98)];
      let extractions = vec![
        extraction(
          98,
          1,
          "2026-06-12T00:00:00Z",
          "2026-06-14T00:00:00Z",
          "2026-06-16T00:00:00Z",
        ),
        extraction(
          77,
          2,
          "2026-06-12T00:00:00Z",
          "2026-06-14T00:00:00Z",
          "2026-06-16T00:00:00Z",
        ),
      ];
      (roster, extractions)
    }

    #[test]
    fn it_filters_to_one_corporation_in_a_corp_scope() {
      let (roster, extractions) = roster_and_extractions();
      let mut state = state_with(Scope::Corp(98), roster, Vec::new());
      state.extractions = extractions;

      let visible = state.visible_extractions();

      assert_eq!(visible.len(), 1);
      assert_eq!(visible[0].corporation_id, 98);
    }

    #[test]
    fn it_resolves_a_character_scope_to_its_corporation() {
      let (roster, extractions) = roster_and_extractions();
      let mut state = state_with(Scope::Char(1), roster, Vec::new());
      state.extractions = extractions;

      let visible = state.visible_extractions();

      assert_eq!(visible.len(), 1);
      assert_eq!(visible[0].corporation_id, 98);
    }

    #[test]
    fn it_shows_every_corporation_in_the_combined_scope() {
      let (roster, extractions) = roster_and_extractions();
      let mut state = state_with(Scope::All, roster, Vec::new());
      state.extractions = extractions;

      assert_eq!(state.visible_extractions().len(), 2);
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
}
