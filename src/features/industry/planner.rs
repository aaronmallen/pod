use std::collections::{BTreeMap, BTreeSet, HashMap};

use iced::Point;

use super::{
  Scope,
  planner_loaders::{self, Category, PlanClone, PlanPilot, PlannerData, PlannerFacility, Recipe},
  planner_model::{
    BuildNode, BuildPlan, MergedBuildJob, PlanSegment, RawTotal, RigFactors, StockAllocation, StockSelection,
    allocate_stock, merge_segments, reconcile_segments, remove_segment, set_segment_assignment, set_segment_runs,
    split_segments,
  },
  rig_bonuses::RigBonus,
};
use crate::{
  features::shell::window_state::UiState,
  store::model::{PlanSegment as RepoPlanSegment, PlanTree, PlanType},
  ui::components::resizable_pane::PaneDrag,
};

pub const DETAIL_PANE_KEY: &str = "industry.planner.detail";

pub const FACILITY_SEARCH_MIN_CHARS: usize = 3;

const DEFAULT_ME: i64 = 10;
const DEFAULT_TE: i64 = 20;
const DETAIL_PANE_DEFAULT_WIDTH: f32 = 340.0;
const DETAIL_PANE_MIN_WIDTH: f32 = 280.0;
const MATERIAL_PLAN_SCROLL_ID: &str = "industry-planner-material-plan";
const ME_MAX: i64 = 10;
const RECENT_LIMIT: usize = 8;
const RUNS_MAX: i64 = 9_999;
const TE_MAX: i64 = 20;

#[derive(Clone, Debug)]
pub struct Economics {
  pub build_time_secs: f64,
  pub install_fee: f64,
  pub margin: f64,
  pub material_cost: f64,
  pub output_qty: i64,
  pub output_volume: f64,
  pub per_unit: f64,
  pub profit: f64,
  pub revenue: f64,
}

impl Economics {
  pub fn isk_per_hour(&self) -> f64 {
    if self.build_time_secs <= 0.0 {
      return 0.0;
    }
    self.profit / (self.build_time_secs / 3_600.0)
  }

  pub fn profitable(&self) -> bool {
    self.profit > 0.0
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FacilityPickerState {
  pub query: String,
  pub results: Vec<PlannerFacility>,
  pub search_generation: u64,
  pub searching: bool,
  pub type_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PinnedStructure {
  pub id: i64,
  pub name: String,
  pub solar_system_id: i64,
  pub type_id: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FacilityDefaults {
  pub manufacturing: Option<i64>,
  pub reactions: Option<i64>,
}

impl FacilityDefaults {
  fn for_activity(&self, is_reaction: bool) -> Option<i64> {
    if is_reaction {
      self.reactions
    } else {
      self.manufacturing
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialMenu {
  pub anchor: Point,
  pub buildable: bool,
  pub built: bool,
  pub mat: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrderMenu {
  pub anchor: Point,
  pub split: bool,
  pub type_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
  BreakDownAll,
  CategorySelected(Category),
  CursorMoved(Point),
  FacilityPickerToggled {
    type_id: i64,
  },
  FacilitySearchChanged {
    query: String,
    type_id: i64,
  },
  FacilitySearchResults {
    generation: u64,
    results: Vec<PlannerFacility>,
    type_id: i64,
  },
  FacilitySelected {
    facility_structure: i64,
    pin: Option<PinnedStructure>,
    solar_system_id: i64,
    type_id: i64,
  },
  MaterialEfficiencyChanged {
    me: i64,
    type_id: i64,
  },
  MaterialRightPressed {
    type_id: i64,
  },
  MenuClosed,
  NodeBrokenDown {
    type_id: i64,
  },
  NodeCollapsed {
    type_id: i64,
  },
  OrderJobMerged {
    type_id: i64,
  },
  OrderJobRightPressed {
    type_id: i64,
  },
  OrderJobSplit {
    type_id: i64,
  },
  OrderMenuClosed,
  OrderPilotAssigned {
    clone_id: Option<i64>,
    index: usize,
    pilot_id: Option<i64>,
    type_id: i64,
  },
  OrderPilotPickerExpanded {
    index: usize,
    pilot_id: i64,
    type_id: i64,
  },
  OrderPilotPickerToggled {
    index: usize,
    type_id: i64,
  },
  OrderSegmentRemoved {
    index: usize,
    type_id: i64,
  },
  OrderSegmentRunsChanged {
    index: usize,
    type_id: i64,
    value: String,
  },
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart,
  PickerScrolled {
    absolute: f32,
  },
  PickerToggled,
  PlanDeleteRequested(i64),
  PlanLoadRequested(i64),
  PlanRestored {
    segments: Vec<RepoPlanSegment>,
    tree: Box<PlanTree>,
  },
  PlanSaveRequested,
  PlansListed(Vec<SavedPlanData>),
  ProductPicked(i64),
  RightTabSelected(RightTab),
  RowCollapseToggled {
    type_id: i64,
  },
  RunsChanged(i64),
  RunsInputChanged(String),
  SearchChanged(String),
  ShoppingListCopied,
  StockSelectionToggled {
    site: i64,
    type_id: i64,
  },
  TimeEfficiencyChanged {
    te: i64,
    type_id: i64,
  },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TypeSettings {
  pub facility_structure: Option<i64>,
  pub facility_system: Option<i64>,
  pub me: i64,
  pub te: i64,
}

#[derive(Debug, Default)]
struct Derived {
  allocation: StockAllocation,
  economics: Option<Economics>,
  merged: Vec<MergedBuildJob>,
  plan: Option<BuildPlan>,
  raw_totals: Vec<RawTotal>,
}

#[derive(Debug)]
pub struct Planner {
  assign_pilots: bool,
  bp_to_product: HashMap<i64, i64>,
  built: BTreeSet<i64>,
  category: Category,
  collapsed_rows: BTreeSet<i64>,
  cursor: Option<Point>,
  data: PlannerData,
  derived: Derived,
  detail_pane: PaneDrag,
  dirty: bool,
  facility_defaults: FacilityDefaults,
  facility_intel: HashMap<i64, Vec<i64>>,
  facility_picker: Option<FacilityPickerState>,
  loaded: bool,
  menu: Option<MaterialMenu>,
  on_hand: HashMap<(i64, i64), i64>,
  order_menu: Option<OrderMenu>,
  order_segments: BTreeMap<i64, Vec<PlanSegment>>,
  pending_blueprint_seed: Option<i64>,
  pilot_picker: Option<(i64, usize, Option<i64>)>,
  pilots: Vec<PlanPilot>,
  picker_open: bool,
  picker_scroll_offset: f32,
  placeholder: String,
  product: Option<i64>,
  recent: Vec<i64>,
  rig_catalog: HashMap<i64, RigBonus>,
  right_tab: RightTab,
  runs: i64,
  runs_input: String,
  saved: Vec<SavedPlan>,
  search: String,
  settings: BTreeMap<i64, TypeSettings>,
  /// The ORDERED list of jobs the user opted to draw from on-hand stock. Order is load-bearing:
  /// [`allocate_stock`] drains each shared `(site, type_id)` pool in this order so no unit is double-counted.
  /// A later UI task appends to this; the netting reads it through [`Planner::stock_allocation`].
  stock_selections: Vec<StockSelection>,
}

impl Planner {
  pub fn new() -> Self {
    Planner {
      assign_pilots: false,
      bp_to_product: HashMap::new(),
      built: BTreeSet::new(),
      category: Category::Other,
      collapsed_rows: BTreeSet::new(),
      cursor: None,
      data: PlannerData::default(),
      derived: Derived::default(),
      detail_pane: PaneDrag::with_min_width(
        DETAIL_PANE_DEFAULT_WIDTH,
        DETAIL_PANE_MIN_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      )
      .right_anchored(true),
      dirty: false,
      facility_defaults: FacilityDefaults::default(),
      facility_intel: HashMap::new(),
      facility_picker: None,
      loaded: false,
      menu: None,
      on_hand: HashMap::new(),
      order_menu: None,
      order_segments: BTreeMap::new(),
      pending_blueprint_seed: None,
      pilot_picker: None,
      pilots: Vec::new(),
      picker_open: false,
      picker_scroll_offset: 0.0,
      placeholder: String::new(),
      product: None,
      recent: Vec::new(),
      rig_catalog: HashMap::new(),
      right_tab: RightTab::default(),
      runs: 1,
      runs_input: "1".to_owned(),
      saved: Vec::new(),
      search: String::new(),
      settings: BTreeMap::new(),
      stock_selections: Vec::new(),
    }
  }

  fn is_plan_affecting(message: &Message) -> bool {
    matches!(
      message,
      Message::BreakDownAll
        | Message::FacilitySelected { .. }
        | Message::MaterialEfficiencyChanged { .. }
        | Message::NodeBrokenDown { .. }
        | Message::NodeCollapsed { .. }
        | Message::ProductPicked(_)
        | Message::RunsChanged(_)
        | Message::RunsInputChanged(_)
        | Message::StockSelectionToggled { .. }
        | Message::TimeEfficiencyChanged { .. }
    )
  }

  pub fn apply_data(&mut self, data: PlannerData) {
    self.bp_to_product = data
      .recipes
      .iter()
      .map(|(&product, recipe)| (recipe.blueprint_type_id, product))
      .collect();
    self.data = data;
    self.loaded = true;
    self.placeholder = t!(
      "industry.planner.search_placeholder_count",
      count => view::fmt_num(self.data.catalog.len() as i64)
    )
    .into_owned();
    if self.recent.is_empty() {
      self.recent = self.seed_recent();
    }
    if let Some(blueprint_type_id) = self.pending_blueprint_seed.take() {
      self.seed_from_blueprint(blueprint_type_id);
    }
    self.recompute();
  }

  pub fn build_sites(&self) -> Vec<i64> {
    let mut sites: BTreeSet<i64> = BTreeSet::new();
    if let Some(product) = self.product
      && let Some(site) = self.settings_for(product).facility_structure
    {
      sites.insert(site);
    }
    for settings in self.settings.values() {
      if let Some(site) = settings.facility_structure {
        sites.insert(site);
      }
    }
    sites.into_iter().collect()
  }

  pub fn assign_pilots(&self) -> bool {
    self.assign_pilots
  }

  pub fn category(&self) -> Category {
    self.category
  }

  pub fn cost_index(&self, type_id: i64) -> Option<f64> {
    let is_reaction = self.data.recipe(type_id).is_some_and(|recipe| recipe.is_reaction);
    self
      .selected_facility(type_id, is_reaction)
      .and_then(|facility| facility.index_for(is_reaction))
  }

  pub fn data(&self) -> &PlannerData {
    &self.data
  }

  pub fn default_facility(&self, is_reaction: bool) -> Option<&PlannerFacility> {
    self
      .data
      .facilities
      .iter()
      .filter(|facility| facility.index_for(is_reaction).is_some())
      .min_by(|a, b| {
        a.index_for(is_reaction)
          .unwrap_or(f64::MAX)
          .total_cmp(&b.index_for(is_reaction).unwrap_or(f64::MAX))
      })
  }

  pub fn detail_pane_ratio(&self) -> f32 {
    self.detail_pane.ratio()
  }

  pub fn detail_pane_width(&self) -> f32 {
    self.detail_pane.width()
  }

  pub fn economics(&self) -> Option<Economics> {
    self.derived.economics.clone()
  }

  fn product_build_time(&self, type_id: i64, recipe: &Recipe) -> f64 {
    let te = self.settings_for(type_id).te;
    let rig_te_factor = self.rig_factors_for(self.settings_for(type_id).facility_structure).te;
    self
      .segments_for(type_id)
      .iter()
      .map(|segment| {
        segment_build_time(
          recipe,
          segment.runs,
          te,
          rig_te_factor,
          self.segment_assignment(segment),
        )
      })
      .sum()
  }

  fn compute_economics(&self, plan: &BuildPlan) -> Option<Economics> {
    let product = self.product?;
    let recipe = self.data.recipe(product)?;

    let material_cost = self.plan_material_cost(plan, &|type_id| self.cost_index(type_id).unwrap_or(0.0));

    let output_qty = recipe.output_per_run * self.runs;
    let revenue = self.data.price(product) * output_qty as f64;
    let eiv = estimated_item_value(&self.data, recipe, self.runs);
    let rig_fee_factor = self.rig_factors_for(self.settings_for(product).facility_structure).fee;
    let install_fee = install_fee(eiv, self.cost_index(product).unwrap_or(0.0), rig_fee_factor);
    let profit = revenue - material_cost - install_fee;
    let margin = if revenue > 0.0 { profit / revenue * 100.0 } else { 0.0 };
    let per_unit = if output_qty > 0 {
      profit / output_qty as f64
    } else {
      0.0
    };
    let build_time_secs = self.product_build_time(product, recipe);

    Some(Economics {
      build_time_secs,
      install_fee,
      margin,
      material_cost,
      output_qty,
      output_volume: self.data.volume(product) * output_qty as f64,
      per_unit,
      profit,
      revenue,
    })
  }

  pub fn facility_picker(&self) -> Option<&FacilityPickerState> {
    self.facility_picker.as_ref()
  }

  pub fn is_dragging_pane(&self) -> bool {
    self.detail_pane.is_active()
  }

  pub fn is_loaded(&self) -> bool {
    self.loaded
  }

  pub fn is_built(&self, type_id: i64) -> bool {
    self.built.contains(&type_id)
  }

  pub fn is_row_collapsed(&self, type_id: i64) -> bool {
    self.collapsed_rows.contains(&type_id)
  }

  pub fn menu(&self) -> Option<&MaterialMenu> {
    self.menu.as_ref()
  }

  pub fn order_menu(&self) -> Option<&OrderMenu> {
    self.order_menu.as_ref()
  }

  pub fn segments_for(&self, type_id: i64) -> Vec<PlanSegment> {
    let stored = self.order_segments.get(&type_id).map(Vec::as_slice).unwrap_or(&[]);
    reconcile_segments(stored, self.total_runs_for(type_id))
  }

  pub fn order_assignment(&self) -> (usize, usize) {
    let mut assigned = 0;
    let mut total = 0;
    for job in self.merged_build_order() {
      let segments = self.segments_for(job.type_id);
      assigned += segments.iter().filter(|segment| segment.pilot_id.is_some()).count();
      total += segments.len();
    }
    (assigned, total)
  }

  pub fn has_plan(&self) -> bool {
    self.derived.plan.is_some()
  }

  pub fn merged_build_order(&self) -> &[MergedBuildJob] {
    &self.derived.merged
  }

  pub fn raw_totals(&self) -> &[RawTotal] {
    &self.derived.raw_totals
  }

  pub fn settings_for(&self, type_id: i64) -> TypeSettings {
    self
      .settings
      .get(&type_id)
      .copied()
      .unwrap_or_else(|| fresh_settings(&self.data, &self.facility_defaults, type_id))
  }

  pub fn picker_open(&self) -> bool {
    self.picker_open
  }

  pub fn pilot(&self, id: i64) -> Option<&PlanPilot> {
    self.pilots.iter().find(|pilot| pilot.id == id)
  }

  pub fn segment_assignment(&self, segment: &PlanSegment) -> Option<(&PlanPilot, Option<&PlanClone>)> {
    if !self.assign_pilots {
      return None;
    }
    let pilot = self.pilot(segment.pilot_id?)?;
    Some((pilot, pilot.clone_named(segment.clone_id)))
  }

  pub fn pilot_picker_open(&self, type_id: i64, index: usize) -> bool {
    self
      .pilot_picker
      .is_some_and(|(open_type, open_index, _)| open_type == type_id && open_index == index)
  }

  pub fn pilot_picker_expanded(&self) -> Option<i64> {
    self.pilot_picker.and_then(|(_, _, pilot_id)| pilot_id)
  }

  pub fn pilots(&self) -> &[PlanPilot] {
    &self.pilots
  }

  pub fn picker_scroll_offset(&self) -> f32 {
    self.picker_scroll_offset
  }

  pub fn plan(&self) -> Option<BuildPlan> {
    self.derived.plan.clone()
  }

  fn compute_plan(&self) -> Option<BuildPlan> {
    let product = self.product?;
    let root = self.assemble(product, &mut BTreeSet::new())?;
    Some(BuildPlan::new(root, self.runs))
  }

  pub fn product(&self) -> Option<i64> {
    self.product
  }

  pub fn has_buildable_inputs(&self) -> bool {
    self
      .product
      .map(|product| !buildable_inputs(&self.data, product).is_empty())
      .unwrap_or(false)
  }

  pub fn seed_from_blueprint(&mut self, blueprint_type_id: i64) -> bool {
    let Some(product) = self.product_for_blueprint(blueprint_type_id) else {
      return false;
    };
    self.select_product(product);
    self.picker_open = false;
    self.search.clear();
    self.category = Category::Other;
    self.push_recent(product);
    self.recompute();
    true
  }

  pub fn queue_blueprint_seed(&mut self, blueprint_type_id: i64) {
    self.pending_blueprint_seed = Some(blueprint_type_id);
  }

  pub fn product_for_blueprint(&self, blueprint_type_id: i64) -> Option<i64> {
    self.bp_to_product.get(&blueprint_type_id).copied()
  }

  pub fn recent(&self) -> &[i64] {
    &self.recent
  }

  pub fn restore(&mut self, tree: &PlanTree) {
    self.product = Some(tree.product_type_id);
    self.set_runs(tree.runs);
    self.settings = tree
      .types
      .iter()
      .map(|kind| {
        (
          kind.type_id,
          TypeSettings {
            facility_structure: kind.facility_structure,
            facility_system: kind.facility_system,
            me: kind.me,
            te: kind.te,
          },
        )
      })
      .collect();
    self.built = tree
      .types
      .iter()
      .filter(|kind| kind.built)
      .map(|kind| kind.type_id)
      .collect();
    self.recompute();
    self.restore_stock_selections(tree);
    self.facility_picker = None;
    self.order_menu = None;
    self.pilot_picker = None;
    self.order_segments.clear();
    self.collapsed_rows.clear();
    self.push_recent(tree.product_type_id);
    self.recompute();
  }

  pub fn restore_segments(&mut self, segments: &[RepoPlanSegment]) {
    self.order_segments.clear();
    for segment in segments {
      self
        .order_segments
        .entry(segment.type_id)
        .or_default()
        .push(PlanSegment {
          clone_id: segment.clone_id,
          pilot_id: segment.pilot_id,
          runs: segment.runs,
        });
    }
  }

  fn restore_stock_selections(&mut self, tree: &PlanTree) {
    self.stock_selections = tree
      .types
      .iter()
      .filter(|kind| kind.use_stock)
      .filter_map(|kind| {
        kind.facility_structure.map(|site| StockSelection {
          needed: self.raw_demand_for(kind.type_id),
          site,
          type_id: kind.type_id,
        })
      })
      .collect();
  }

  pub fn right_tab(&self) -> RightTab {
    self.right_tab
  }

  pub fn runs(&self) -> i64 {
    self.runs
  }

  pub fn runs_input(&self) -> &str {
    &self.runs_input
  }

  pub fn save_name(&self) -> Option<String> {
    let product = self.product?;
    Some(format!("{} \u{00D7}{}", self.data.name(product), self.runs))
  }

  pub fn saved(&self) -> &[SavedPlan] {
    &self.saved
  }

  pub fn search(&self) -> &str {
    &self.search
  }

  pub fn search_placeholder(&self) -> &str {
    if self.placeholder.is_empty() {
      "Search buildable products\u{2026}"
    } else {
      &self.placeholder
    }
  }

  pub fn set_assign_pilots(&mut self, enabled: bool) {
    self.assign_pilots = enabled;
    if !enabled {
      self.pilot_picker = None;
      self.pilots.clear();
    }
  }

  pub fn set_facility_defaults(&mut self, defaults: FacilityDefaults) {
    self.facility_defaults = defaults;
  }

  pub fn set_rig_data(&mut self, facility_intel: HashMap<i64, Vec<i64>>, rig_catalog: HashMap<i64, RigBonus>) {
    self.facility_intel = facility_intel;
    self.rig_catalog = rig_catalog;
    self.recompute();
  }

  pub fn set_pilots(&mut self, pilots: Vec<PlanPilot>) {
    if self.assign_pilots {
      self.pilots = pilots;
    }
  }

  pub fn set_on_hand(&mut self, on_hand: HashMap<(i64, i64), i64>) {
    self.on_hand = on_hand;
    self.recompute();
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.detail_pane.set_host_width(host_width);
  }

  pub fn selected_facility(&self, type_id: i64, is_reaction: bool) -> Option<&PlannerFacility> {
    let settings = self.settings_for(type_id);
    // Resolve the EXACT picked structure first. A system can host both a manufacturing and a reaction
    // structure (e.g. a manufacturing array and a reaction array in the same system); resolving by system
    // alone returned whichever the facility list sorted first (manufacturing, by cost index), so a reaction's
    // configured site — and any manual pick — silently displayed as the wrong facility. The structure id is
    // the user's actual choice, so honor it before falling back to the system, then the activity default.
    if let Some(structure) = settings.facility_structure
      && let Some(facility) = self.data.facilities.iter().find(|f| f.id == structure)
    {
      return Some(facility);
    }
    match settings.facility_system {
      Some(system) => self
        .data
        .facilities
        .iter()
        .find(|f| f.solar_system_id == system && f.index_for(is_reaction).is_some())
        .or_else(|| self.data.facilities.iter().find(|f| f.solar_system_id == system))
        .or_else(|| self.default_facility(is_reaction)),
      None => self.default_facility(is_reaction),
    }
  }

  fn rig_factors_for(&self, structure_id: Option<i64>) -> RigFactors {
    let Some(structure_id) = structure_id else {
      return RigFactors::default();
    };
    let Some(rigs) = self.facility_intel.get(&structure_id) else {
      return RigFactors::default();
    };
    if rigs.is_empty() {
      return RigFactors::default();
    }
    let security_status = self
      .data
      .facilities
      .iter()
      .find(|facility| facility.id == structure_id)
      .and_then(|facility| facility.security_status)
      .unwrap_or(1.0);
    RigFactors::from_rigs(rigs, &self.rig_catalog, security_status)
  }

  pub fn stock_allocation(&self) -> StockAllocation {
    self.derived.allocation.clone()
  }

  pub fn is_stock_selected(&self, site: i64, type_id: i64) -> bool {
    self
      .stock_selections
      .iter()
      .any(|selection| selection.site == site && selection.type_id == type_id)
  }

  pub fn on_hand_at(&self, site: i64, type_id: i64) -> i64 {
    self.on_hand.get(&(site, type_id)).copied().unwrap_or(0)
  }

  pub fn remaining_pool(&self, site: i64, type_id: i64) -> i64 {
    let drawn = self
      .derived
      .allocation
      .drawn_by_pool
      .get(&(site, type_id))
      .copied()
      .unwrap_or(0);
    (self.on_hand_at(site, type_id) - drawn).max(0)
  }

  pub fn shopping_list(&self) -> String {
    let Some(plan) = self.plan() else {
      return String::new();
    };
    let mut totals = plan.raw_totals_after_stock(&self.stock_allocation());
    totals.sort_by(|a, b| {
      let cost_a = a.qty as f64 * self.data.price(a.type_id);
      let cost_b = b.qty as f64 * self.data.price(b.type_id);
      cost_b.total_cmp(&cost_a)
    });
    totals
      .iter()
      .map(|total| format!("{}\t{}", total.qty, self.data.name(total.type_id)))
      .collect::<Vec<_>>()
      .join("\n")
  }

  pub fn snapshot(&self) -> Option<PlanTree> {
    let product = self.product?;
    let ids: BTreeSet<i64> = std::iter::once(product)
      .chain(self.settings.keys().copied())
      .chain(self.built.iter().copied())
      .chain(self.stock_selections.iter().map(|selection| selection.type_id))
      .collect();
    let types = ids
      .into_iter()
      .map(|type_id| {
        let settings = self.settings_for(type_id);
        let stock_site = self
          .stock_selections
          .iter()
          .find(|selection| selection.type_id == type_id)
          .map(|selection| selection.site);
        PlanType {
          built: self.built.contains(&type_id),
          facility_structure: stock_site.or(settings.facility_structure),
          facility_system: settings.facility_system,
          me: settings.me,
          te: settings.te,
          type_id,
          use_stock: stock_site.is_some(),
        }
      })
      .collect();
    Some(PlanTree {
      product_type_id: product,
      root_facility_system: self.settings_for(product).facility_system,
      runs: self.runs,
      types,
    })
  }

  pub fn segments(&self) -> Vec<RepoPlanSegment> {
    let mut out = Vec::new();
    for &type_id in self.order_segments.keys() {
      let segments = self.segments_for(type_id);
      let trivial = segments.len() == 1 && segments[0].pilot_id.is_none() && segments[0].clone_id.is_none();
      if trivial {
        continue;
      }
      for (index, segment) in segments.iter().enumerate() {
        out.push(RepoPlanSegment {
          clone_id: segment.clone_id,
          pilot_id: segment.pilot_id,
          runs: segment.runs,
          segment_index: index as i64,
          type_id,
        });
      }
    }
    out
  }

  pub fn with_restored_panes(mut self, ui: &UiState) -> Self {
    let host_width = ui.host_width("main", crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH);
    self.detail_pane = PaneDrag::from_store_with_min(
      ui,
      DETAIL_PANE_KEY,
      DETAIL_PANE_DEFAULT_WIDTH,
      DETAIL_PANE_MIN_WIDTH,
      host_width,
    )
    .right_anchored(true);
    self
  }

  pub fn update(&mut self, message: Message) {
    self.dirty |= Self::is_plan_affecting(&message);
    match message {
      Message::BreakDownAll => self.break_down_all(),
      Message::CategorySelected(category) => {
        self.category = category;
        self.picker_scroll_offset = 0.0;
      }
      Message::CursorMoved(point) => self.cursor = Some(point),
      Message::FacilityPickerToggled {
        ..
      }
      | Message::FacilitySearchChanged {
        ..
      }
      | Message::FacilitySearchResults {
        ..
      }
      | Message::FacilitySelected {
        ..
      } => self.update_facility(message),
      Message::MaterialEfficiencyChanged {
        ..
      }
      | Message::TimeEfficiencyChanged {
        ..
      } => self.update_efficiency(message),
      Message::MaterialRightPressed {
        ..
      }
      | Message::MenuClosed
      | Message::NodeBrokenDown {
        ..
      }
      | Message::NodeCollapsed {
        ..
      } => self.update_menu(message),
      Message::OrderJobMerged {
        ..
      }
      | Message::OrderJobRightPressed {
        ..
      }
      | Message::OrderJobSplit {
        ..
      }
      | Message::OrderMenuClosed
      | Message::OrderPilotAssigned {
        ..
      }
      | Message::OrderPilotPickerExpanded {
        ..
      }
      | Message::OrderPilotPickerToggled {
        ..
      }
      | Message::OrderSegmentRemoved {
        ..
      }
      | Message::OrderSegmentRunsChanged {
        ..
      } => self.update_order(message),
      Message::PaneDrag(x) => {
        self.detail_pane.drag_to(x);
      }
      Message::PaneDragEnd => self.detail_pane.end(),
      Message::PaneDragStart => self.detail_pane.start(),
      Message::PickerScrolled {
        absolute,
      } => self.picker_scroll_offset = absolute,
      Message::PickerToggled => self.picker_open = !self.picker_open,
      Message::PlanDeleteRequested(_) | Message::PlanLoadRequested(_) | Message::PlanSaveRequested => {}
      Message::PlanRestored {
        segments,
        tree,
      } => {
        self.restore(&tree);
        self.restore_segments(&segments);
        self.right_tab = RightTab::Detail;
      }
      Message::PlansListed(plans) => self.apply_saved(plans),
      Message::ProductPicked(type_id) => {
        self.select_product(type_id);
        self.picker_open = false;
        self.search.clear();
        self.category = Category::Other;
        self.push_recent(type_id);
      }
      Message::RightTabSelected(tab) => self.right_tab = tab,
      Message::RowCollapseToggled {
        type_id,
      } => self.toggle_row_collapse(type_id),
      Message::RunsChanged(runs) => self.set_runs(runs),
      Message::RunsInputChanged(raw) => self.edit_runs(raw),
      Message::SearchChanged(query) => {
        self.search = query;
        self.picker_scroll_offset = 0.0;
      }
      Message::ShoppingListCopied => {}
      Message::StockSelectionToggled {
        site,
        type_id,
      } => self.toggle_stock_selection(site, type_id),
    }
    if self.dirty {
      self.recompute();
    }
  }

  fn update_facility(&mut self, message: Message) {
    match message {
      Message::FacilityPickerToggled {
        type_id,
      } => self.toggle_facility_picker(type_id),
      Message::FacilitySearchChanged {
        query,
        type_id,
      } => self.edit_facility_query(type_id, query),
      Message::FacilitySearchResults {
        generation,
        results,
        type_id,
      } => self.apply_facility_results(generation, type_id, results),
      Message::FacilitySelected {
        facility_structure,
        solar_system_id,
        type_id,
        ..
      } => {
        let settings = self.settings_mut(type_id);
        settings.facility_structure = Some(facility_structure);
        settings.facility_system = Some(solar_system_id);
        self.facility_picker = None;
      }
      _ => {}
    }
  }

  fn edit_facility_query(&mut self, type_id: i64, query: String) {
    let live = query.trim().chars().count() >= FACILITY_SEARCH_MIN_CHARS;
    match self.facility_picker.as_mut().filter(|state| state.type_id == type_id) {
      Some(state) => {
        state.query = query;
        state.search_generation = state.search_generation.wrapping_add(1);
        state.searching = live;
        if !live {
          state.results.clear();
        }
      }
      None => {
        self.facility_picker = Some(FacilityPickerState {
          query,
          results: Vec::new(),
          search_generation: 1,
          searching: live,
          type_id,
        })
      }
    }
  }

  fn apply_facility_results(&mut self, generation: u64, type_id: i64, results: Vec<PlannerFacility>) {
    if let Some(state) = self
      .facility_picker
      .as_mut()
      .filter(|state| state.type_id == type_id && state.search_generation == generation)
    {
      state.results = results;
      state.searching = false;
    }
  }

  fn update_efficiency(&mut self, message: Message) {
    match message {
      Message::MaterialEfficiencyChanged {
        me,
        type_id,
      } => self.settings_mut(type_id).me = me.clamp(0, ME_MAX),
      Message::TimeEfficiencyChanged {
        te,
        type_id,
      } => self.settings_mut(type_id).te = te.clamp(0, TE_MAX),
      _ => {}
    }
  }

  fn update_menu(&mut self, message: Message) {
    match message {
      Message::MaterialRightPressed {
        type_id,
      } => self.open_menu(type_id),
      Message::MenuClosed => self.menu = None,
      Message::NodeBrokenDown {
        type_id,
      } => {
        self.break_down(type_id);
        self.menu = None;
      }
      Message::NodeCollapsed {
        type_id,
      } => {
        self.built.remove(&type_id);
        self.menu = None;
      }
      _ => {}
    }
  }

  fn update_order(&mut self, message: Message) {
    match message {
      Message::OrderJobMerged {
        type_id,
      } => {
        self.merge_order_job(type_id);
        self.order_menu = None;
      }
      Message::OrderJobRightPressed {
        type_id,
      } => self.open_order_menu(type_id),
      Message::OrderJobSplit {
        type_id,
      } => {
        self.split_order_job(type_id);
        self.order_menu = None;
      }
      Message::OrderMenuClosed => self.order_menu = None,
      Message::OrderPilotAssigned {
        clone_id,
        index,
        pilot_id,
        type_id,
      } => {
        self.assign_pilot(type_id, index, pilot_id, clone_id);
        self.pilot_picker = None;
      }
      Message::OrderPilotPickerExpanded {
        index,
        pilot_id,
        type_id,
      } => self.expand_pilot(type_id, index, pilot_id),
      Message::OrderPilotPickerToggled {
        index,
        type_id,
      } => self.toggle_pilot_picker(type_id, index),
      Message::OrderSegmentRemoved {
        index,
        type_id,
      } => self.remove_order_segment(type_id, index),
      Message::OrderSegmentRunsChanged {
        index,
        type_id,
        value,
      } => self.edit_segment_runs(type_id, index, value),
      _ => {}
    }
  }

  fn apply_saved(&mut self, plans: Vec<SavedPlanData>) {
    self.saved = plans
      .into_iter()
      .map(|plan| {
        let economics = self.tree_economics(&plan.tree);
        SavedPlan {
          economics,
          id: plan.id,
          name: plan.name,
          product_type_id: plan.tree.product_type_id,
        }
      })
      .collect();
  }

  fn assemble(&self, type_id: i64, seen: &mut BTreeSet<i64>) -> Option<BuildNode> {
    self.assemble_from(type_id, &self.settings, &self.built, seen)
  }

  fn assemble_from(
    &self,
    type_id: i64,
    settings: &BTreeMap<i64, TypeSettings>,
    built: &BTreeSet<i64>,
    seen: &mut BTreeSet<i64>,
  ) -> Option<BuildNode> {
    let recipe = self.data.recipe(type_id)?;
    if !seen.insert(type_id) {
      return None;
    }
    let config = settings
      .get(&type_id)
      .copied()
      .unwrap_or_else(|| self.fresh_settings(type_id));
    let mut node = BuildNode::new(
      type_id,
      recipe.output_per_run,
      recipe.is_reaction,
      recipe.materials.clone(),
    );
    node.facility = config.facility_system;
    node.facility_structure = config.facility_structure;
    node.me = if recipe.is_reaction { 0 } else { config.me };
    node.te = if recipe.is_reaction { 0 } else { config.te };
    let rig_factors = self.rig_factors_for(config.facility_structure);
    node.rig_fee_factor = rig_factors.fee;
    node.rig_me_factor = rig_factors.me;
    node.rig_te_factor = rig_factors.te;
    let materials: Vec<i64> = recipe.materials.iter().map(|material| material.type_id).collect();
    for mat in materials {
      if built.contains(&mat)
        && let Some(child) = self.assemble_from(mat, settings, built, seen)
      {
        node.children.insert(mat, child);
      }
    }
    seen.remove(&type_id);
    Some(node)
  }

  fn break_down(&mut self, mat: i64) {
    if self.data.recipe(mat).is_none() {
      return;
    }
    let fresh = self.fresh_settings(mat);
    self.settings.entry(mat).or_insert(fresh);
    self.built.insert(mat);
  }

  fn break_down_all(&mut self) {
    let Some(product) = self.product else {
      return;
    };
    let mut seen = BTreeSet::new();
    self.break_down_descendants(product, &mut seen);
  }

  fn break_down_descendants(&mut self, type_id: i64, seen: &mut BTreeSet<i64>) {
    if !seen.insert(type_id) {
      return;
    }
    for mat in buildable_inputs(&self.data, type_id) {
      self.break_down(mat);
      self.break_down_descendants(mat, seen);
    }
  }

  fn cost_index_for(&self, facility_system: Option<i64>, is_reaction: bool) -> f64 {
    facility_system
      .and_then(|system| {
        self
          .data
          .facilities
          .iter()
          .find(|facility| facility.solar_system_id == system)
      })
      .or_else(|| self.default_facility(is_reaction))
      .and_then(|facility| facility.index_for(is_reaction))
      .unwrap_or(0.0)
  }

  fn edit_runs(&mut self, raw: String) {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    self.runs = digits.parse::<i64>().unwrap_or(1).clamp(1, RUNS_MAX);
    self.runs_input = digits;
  }

  fn fresh_settings(&self, type_id: i64) -> TypeSettings {
    fresh_settings(&self.data, &self.facility_defaults, type_id)
  }

  fn edit_segment_runs(&mut self, type_id: i64, index: usize, raw: String) {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    let value = digits.parse::<i64>().unwrap_or(1);
    let total = self.total_runs_for(type_id);
    let stored = self.order_segments.get(&type_id).map(Vec::as_slice).unwrap_or(&[]);
    self
      .order_segments
      .insert(type_id, set_segment_runs(stored, total, index, value));
  }

  fn merge_order_job(&mut self, type_id: i64) {
    let total = self.total_runs_for(type_id);
    let stored = self.order_segments.get(&type_id).map(Vec::as_slice).unwrap_or(&[]);
    self.order_segments.insert(type_id, merge_segments(stored, total));
  }

  fn open_menu(&mut self, type_id: i64) {
    let Some(anchor) = self.cursor else {
      return;
    };
    self.menu = Some(MaterialMenu {
      anchor,
      buildable: self.data.recipe(type_id).is_some(),
      built: self.built.contains(&type_id),
      mat: type_id,
    });
  }

  fn open_order_menu(&mut self, type_id: i64) {
    let Some(anchor) = self.cursor else {
      return;
    };
    self.order_menu = Some(OrderMenu {
      anchor,
      split: self.segments_for(type_id).len() > 1,
      type_id,
    });
  }

  fn assign_pilot(&mut self, type_id: i64, index: usize, pilot_id: Option<i64>, clone_id: Option<i64>) {
    if !self.assign_pilots {
      return;
    }
    let total = self.total_runs_for(type_id);
    let stored = self.order_segments.get(&type_id).map(Vec::as_slice).unwrap_or(&[]);
    self.order_segments.insert(
      type_id,
      set_segment_assignment(stored, total, index, pilot_id, clone_id),
    );
  }

  fn remove_order_segment(&mut self, type_id: i64, index: usize) {
    let total = self.total_runs_for(type_id);
    let stored = self.order_segments.get(&type_id).map(Vec::as_slice).unwrap_or(&[]);
    self
      .order_segments
      .insert(type_id, remove_segment(stored, total, index));
  }

  fn toggle_pilot_picker(&mut self, type_id: i64, index: usize) {
    if self.pilot_picker_open(type_id, index) {
      self.pilot_picker = None;
    } else {
      self.pilot_picker = Some((type_id, index, None));
    }
  }

  fn expand_pilot(&mut self, type_id: i64, index: usize, pilot_id: i64) {
    if self.pilot_picker_open(type_id, index) {
      self.pilot_picker = Some((type_id, index, Some(pilot_id)));
    }
  }

  fn split_order_job(&mut self, type_id: i64) {
    let total = self.total_runs_for(type_id);
    let stored = self.order_segments.get(&type_id).map(Vec::as_slice).unwrap_or(&[]);
    self.order_segments.insert(type_id, split_segments(stored, total));
  }

  fn total_runs_for(&self, type_id: i64) -> i64 {
    self
      .derived
      .plan
      .as_ref()
      .map(|plan| plan.total_runs_for(type_id))
      .unwrap_or(0)
  }

  fn plan_material_cost(&self, plan: &BuildPlan, cost_index: &dyn Fn(i64) -> f64) -> f64 {
    let acquisition: f64 = plan
      .raw_totals()
      .iter()
      .map(|total| total.qty as f64 * self.data.price(total.type_id))
      .sum();

    let sub_fees: f64 = plan
      .merged_build_order()
      .iter()
      .filter(|job| !job.is_root)
      .map(|job| {
        let eiv = node_eiv(&self.data, &job.node, job.runs);
        install_fee(eiv, cost_index(job.type_id), job.node.rig_fee_factor)
      })
      .sum();

    acquisition + sub_fees
  }

  fn settings_mut(&mut self, type_id: i64) -> &mut TypeSettings {
    let fresh = self.fresh_settings(type_id);
    self.settings.entry(type_id).or_insert(fresh)
  }

  fn push_recent(&mut self, type_id: i64) {
    self.recent.retain(|&id| id != type_id);
    self.recent.insert(0, type_id);
    self.recent.truncate(RECENT_LIMIT);
  }

  fn recompute(&mut self) {
    let plan = self.compute_plan();
    let merged = plan.as_ref().map(BuildPlan::merged_build_order).unwrap_or_default();
    let raw_totals = plan.as_ref().map(BuildPlan::raw_totals).unwrap_or_default();
    let allocation = allocate_stock(&self.on_hand, &self.stock_selections);
    let economics = plan.as_ref().and_then(|plan| self.compute_economics(plan));
    self.derived = Derived {
      allocation,
      economics,
      merged,
      plan,
      raw_totals,
    };
    self.dirty = false;
  }

  fn seed_recent(&self) -> Vec<i64> {
    let mut owned: Vec<i64> = self
      .data
      .catalog
      .iter()
      .filter(|entry| self.data.owned.contains_key(&entry.type_id))
      .map(|entry| entry.type_id)
      .take(RECENT_LIMIT)
      .collect();
    if owned.is_empty() {
      owned = self.data.catalog.iter().take(RECENT_LIMIT).map(|e| e.type_id).collect();
    }
    owned
  }

  fn select_product(&mut self, type_id: i64) {
    self.product = Some(type_id);
    self.settings.clear();
    self.built.clear();
    self.stock_selections.clear();
    self.collapsed_rows.clear();
    self.order_segments.clear();
    self.order_menu = None;
    self.pilot_picker = None;
    self.facility_picker = None;
  }

  fn set_runs(&mut self, runs: i64) {
    self.runs = runs.clamp(1, RUNS_MAX);
    self.runs_input = self.runs.to_string();
  }

  fn toggle_stock_selection(&mut self, site: i64, type_id: i64) {
    if self.is_stock_selected(site, type_id) {
      self
        .stock_selections
        .retain(|selection| !(selection.site == site && selection.type_id == type_id));
      return;
    }
    self.stock_selections.push(StockSelection {
      needed: self.raw_demand_for(type_id),
      site,
      type_id,
    });
  }

  fn raw_demand_for(&self, type_id: i64) -> i64 {
    self
      .derived
      .raw_totals
      .iter()
      .find(|total| total.type_id == type_id)
      .map(|total| total.qty)
      .unwrap_or(0)
  }

  fn toggle_row_collapse(&mut self, type_id: i64) {
    if !self.collapsed_rows.insert(type_id) {
      self.collapsed_rows.remove(&type_id);
    }
  }

  fn toggle_facility_picker(&mut self, type_id: i64) {
    if self
      .facility_picker
      .as_ref()
      .is_some_and(|state| state.type_id == type_id)
    {
      self.facility_picker = None;
    } else {
      self.facility_picker = Some(FacilityPickerState {
        query: String::new(),
        results: Vec::new(),
        search_generation: 0,
        searching: false,
        type_id,
      });
    }
  }

  fn tree_economics(&self, tree: &PlanTree) -> Option<Economics> {
    let product = tree.product_type_id;
    let recipe = self.data.recipe(product)?;
    let runs = tree.runs.clamp(1, RUNS_MAX);
    let settings: BTreeMap<i64, TypeSettings> = tree
      .types
      .iter()
      .map(|kind| {
        (
          kind.type_id,
          TypeSettings {
            facility_structure: kind.facility_structure,
            facility_system: kind.facility_system,
            me: kind.me,
            te: kind.te,
          },
        )
      })
      .collect();
    let built: BTreeSet<i64> = tree
      .types
      .iter()
      .filter(|kind| kind.built)
      .map(|kind| kind.type_id)
      .collect();
    let te = settings.get(&product).map(|kind| kind.te).unwrap_or(0);
    let rig_factors = self.rig_factors_for(settings.get(&product).and_then(|kind| kind.facility_structure));

    let root = self.assemble_from(product, &settings, &built, &mut BTreeSet::new())?;
    let plan = BuildPlan::new(root, runs);
    let material_cost = self.plan_material_cost(&plan, &|type_id| {
      let is_reaction = self.data.recipe(type_id).is_some_and(|recipe| recipe.is_reaction);
      self.cost_index_for(
        settings.get(&type_id).and_then(|kind| kind.facility_system),
        is_reaction,
      )
    });

    let output_qty = recipe.output_per_run * runs;
    let revenue = self.data.price(product) * output_qty as f64;
    let cost_index = self.cost_index_for(tree.root_facility_system, recipe.is_reaction);
    let eiv = estimated_item_value(&self.data, recipe, runs);
    let install_fee = install_fee(eiv, cost_index, rig_factors.fee);
    let profit = revenue - material_cost - install_fee;
    let margin = if revenue > 0.0 { profit / revenue * 100.0 } else { 0.0 };
    let per_unit = if output_qty > 0 {
      profit / output_qty as f64
    } else {
      0.0
    };
    let build_time_secs = node_build_time(recipe, runs, te, rig_factors.te);

    Some(Economics {
      build_time_secs,
      install_fee,
      margin,
      material_cost,
      output_qty,
      output_volume: self.data.volume(product) * output_qty as f64,
      per_unit,
      profit,
      revenue,
    })
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RightTab {
  #[default]
  Detail,
  Plans,
}

#[derive(Clone, Debug)]
pub struct SavedPlan {
  pub economics: Option<Economics>,
  pub id: i64,
  pub name: String,
  pub product_type_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavedPlanData {
  pub id: i64,
  pub name: String,
  pub tree: PlanTree,
}

fn fresh_settings(data: &PlannerData, defaults: &FacilityDefaults, type_id: i64) -> TypeSettings {
  let is_reaction = data.recipe(type_id).is_some_and(|recipe| recipe.is_reaction);
  let owned = data.owned.get(&type_id);
  let (me, te) = if is_reaction {
    (0, 0)
  } else {
    (
      owned.map(|bp| bp.material_efficiency).unwrap_or(DEFAULT_ME),
      owned.map(|bp| bp.time_efficiency).unwrap_or(DEFAULT_TE),
    )
  };
  let (facility_structure, facility_system) = default_facility_for(data, defaults, is_reaction);
  TypeSettings {
    facility_structure,
    facility_system,
    me,
    te,
  }
}

fn default_facility_for(
  data: &PlannerData,
  defaults: &FacilityDefaults,
  is_reaction: bool,
) -> (Option<i64>, Option<i64>) {
  let Some(facility_id) = defaults.for_activity(is_reaction) else {
    return (None, None);
  };
  data
    .facilities
    .iter()
    .find(|facility| facility.id == facility_id)
    .map(|facility| (Some(facility.id), Some(facility.solar_system_id)))
    .unwrap_or((None, None))
}

fn buildable_inputs(data: &PlannerData, type_id: i64) -> Vec<i64> {
  let Some(recipe) = data.recipe(type_id) else {
    return Vec::new();
  };
  recipe
    .materials
    .iter()
    .map(|material| material.type_id)
    .filter(|&mat| data.recipe(mat).is_some())
    .collect()
}

// EIV uses pre-ME base quantities and CCP adjusted prices, never the live market or ME-reduced demand:
// ME affects the materials consumed, not the value EVE taxes the job against.
fn estimated_item_value(data: &PlannerData, recipe: &Recipe, runs: i64) -> f64 {
  let per_run: f64 = recipe
    .materials
    .iter()
    .map(|material| material.base_qty as f64 * data.adjusted_price(material.type_id))
    .sum();
  per_run * runs as f64
}

fn node_eiv(data: &PlannerData, node: &BuildNode, runs: i64) -> f64 {
  let per_run: f64 = node
    .materials
    .iter()
    .map(|material| material.base_qty as f64 * data.adjusted_price(material.type_id))
    .sum();
  per_run * runs as f64
}

fn install_fee(eiv: f64, cost_index: f64, rig_fee_factor: f64) -> f64 {
  install_fee_with_facility_tax(eiv, cost_index, rig_fee_factor, planner_loaders::FACILITY_TAX_RATE)
}

fn install_fee_with_facility_tax(eiv: f64, cost_index: f64, rig_fee_factor: f64, facility_tax_rate: f64) -> f64 {
  let gross_cost = eiv * cost_index * rig_fee_factor;
  let facility_tax = eiv * facility_tax_rate;
  let scc_surcharge = eiv * planner_loaders::SCC_SURCHARGE_RATE;
  gross_cost + facility_tax + scc_surcharge
}

pub fn node_build_time(recipe: &Recipe, runs: i64, te: i64, rig_te_factor: f64) -> f64 {
  let base = recipe.time_per_run as f64 * runs as f64;
  if recipe.is_reaction {
    base * rig_te_factor
  } else {
    base * (1.0 - te as f64 / 100.0) * rig_te_factor
  }
}

pub fn segment_build_time(
  recipe: &Recipe,
  runs: i64,
  te: i64,
  rig_te_factor: f64,
  assignment: Option<(&PlanPilot, Option<&PlanClone>)>,
) -> f64 {
  let base = node_build_time(recipe, runs, te, rig_te_factor);
  match assignment {
    None => base,
    Some((pilot, clone)) => {
      let skill = pilot.skill_time_multiplier(recipe.is_reaction);
      let implant = 1.0 - clone.map(|clone| clone.time_bonus(recipe.is_reaction)).unwrap_or(0.0) / 100.0;
      base * skill * implant
    }
  }
}

pub fn load(
  db: crate::store::Database,
  scope: Scope,
  catalog: Option<planner_loaders::StaticCatalog>,
) -> iced::Task<PlannerData> {
  iced::Task::perform(
    async move {
      match catalog {
        Some(catalog) => planner_loaders::load_data_with_catalog(&db, scope, catalog).await,
        None => planner_loaders::load_data(&db, scope).await,
      }
    },
    |data| data,
  )
}

pub fn view<'a>(planner: &'a Planner, _scope: Scope) -> iced::Element<'a, Message> {
  use iced::{
    Length,
    widget::{Space, Stack, mouse_area},
  };

  if !planner.is_loaded() {
    return view::loading();
  }

  let base = mouse_area(view::body(planner)).on_move(Message::CursorMoved).into();

  // The Material Plan scrollable lives inside `base`. The root element must keep a stable widget
  // identity whether or not an overlay is open, otherwise iced rebuilds the scrollable and resets
  // its offset to the top. Always return the same Stack shape — an empty overlay slot when nothing
  // is open — so opening the material context menu and breaking a node down move the scroll position
  // not at all. The "Build at" facility picker no longer lives here: it floats via AnchoredDropdown
  // anchored under its trigger inside `base`, so it adds no overlay layer and never reshapes this Stack.
  let overlay: iced::Element<'a, Message> = if let Some(menu) = planner.menu() {
    material_menu_overlay(planner, menu)
  } else if let Some(menu) = planner.order_menu() {
    order_menu_overlay(planner, menu)
  } else {
    Space::new().width(Length::Shrink).height(Length::Shrink).into()
  };

  Stack::with_children(vec![base, overlay])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn material_menu_overlay<'a>(planner: &'a Planner, menu: &MaterialMenu) -> iced::Element<'a, Message> {
  use iced::{Length, widget::Stack};

  use crate::ui::components::{backdrop, context_menu};

  let item = if !menu.buildable {
    context_menu::Item::disabled(t!("industry.planner.context_raw_material"))
  } else if menu.built {
    context_menu::Item::action(
      t!("industry.planner.context_stop_building"),
      Message::NodeCollapsed {
        type_id: menu.mat,
      },
    )
  } else {
    context_menu::Item::warning(
      t!("industry.planner.context_break_down"),
      Message::NodeBrokenDown {
        type_id: menu.mat,
      },
    )
  };

  let title = planner.data().name(menu.mat);
  Stack::with_children(vec![
    backdrop::click_catcher(Message::MenuClosed),
    context_menu::context_menu(&title, vec![item], menu.anchor),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn order_menu_overlay<'a>(planner: &'a Planner, menu: &OrderMenu) -> iced::Element<'a, Message> {
  use iced::{Length, widget::Stack};

  use crate::ui::components::{backdrop, context_menu};

  let segments = planner.segments_for(menu.type_id);
  let segment_count = segments.len();
  let total: i64 = segments.iter().map(|segment| segment.runs).sum();
  let mut items = vec![order_split_item(menu, total, segment_count)];
  if menu.split {
    items.push(context_menu::Item::action(
      t!("industry.planner.context_merge"),
      Message::OrderJobMerged {
        type_id: menu.type_id,
      },
    ));
  }

  let title = planner.data().name(menu.type_id);
  Stack::with_children(vec![
    backdrop::click_catcher(Message::OrderMenuClosed),
    context_menu::context_menu(&title, items, menu.anchor),
  ])
  .width(Length::Fill)
  .height(Length::Fill)
  .into()
}

fn order_split_item(
  menu: &OrderMenu,
  total: i64,
  segment_count: usize,
) -> crate::ui::components::context_menu::Item<Message> {
  use crate::ui::components::context_menu;

  if total <= segment_count as i64 {
    return context_menu::Item::disabled(t!("industry.planner.context_split_too_few"));
  }
  let label = if menu.split {
    t!("industry.planner.context_split_again")
  } else {
    t!("industry.planner.context_split_two")
  };
  context_menu::Item::action(
    label,
    Message::OrderJobSplit {
      type_id: menu.type_id,
    },
  )
}

mod view {
  use iced::{
    Background, Border, ContentFit, Element, Length, Padding,
    alignment::{Horizontal, Vertical},
    widget::{Column, Row, Space, button, container, image, mouse_area, scrollable, slider, text, text_input},
  };

  use super::{
    Economics, MATERIAL_PLAN_SCROLL_ID, Message, Planner, RightTab, SavedPlan, node_build_time, segment_build_time,
  };
  use crate::{
    features::industry::{
      planner_loaders::{Category, OwnedSummary, PlanClone, PlanPilot, PlannerData, PlannerFacility, Recipe},
      planner_model::{MergedBuildJob, NeededBlueprint, PlanSegment, eff_qty, needed_blueprints_from, runs_for},
    },
    store::images::IconResolution,
    ui::{
      components::{
        anchored_dropdown::AnchoredDropdown,
        avatar::avatar,
        badge::badge,
        button::{Button, Size},
        clip::clip_layer,
        facility_combobox::{FacilityCombobox, FacilityRef},
        icon::Icon,
        icon_tile::icon_tile,
        resizable_pane::pane_handle,
        rule,
        tab_select::{Tab, TabLayout, tab_select_with},
        text_input::{TextInput, inner_style as text_input_inner_style},
        virtual_list::{self, VirtualList, VirtualListConfig},
      },
      format::{fmt_duration_coarse, fmt_isk, fmt_isk_full, fmt_volume},
      style::{color, radius, spacing, typography},
    },
  };

  const ASSIGN_SLOT_HEIGHT: f32 = 34.0;
  const ASSIGN_SLOT_WIDTH: f32 = 200.0;
  const ESTIMATED_PICKER_ROW: f32 = 52.0;
  const FACILITY_PICKER_WIDTH: f32 = 450.0;
  const SEGMENT_INDENT: f32 = 64.0;
  const SEGMENT_REMOVE_BOX: f32 = 24.0;
  const SEGMENT_RUNS_FIELD_WIDTH: f32 = 46.0;
  /// Smallest id EVE assigns a player-owned structure; NPC stations sit well below it. A live result at or
  /// above this id is a structure that must be pinned (persisted) when selected, since it never reaches the
  /// SDE/corp-sync facility tables.
  const MIN_STRUCTURE_ID: i64 = 1_000_000_000_000;
  const PANE_PADDING: f32 = 24.0;
  const PICKER_MAX_RESULTS: usize = 200;
  const PILOT_PICKER_MAX_HEIGHT: f32 = 360.0;
  const PILOT_PICKER_WIDTH: f32 = 280.0;
  const RUNS_FIELD_WIDTH: f32 = 34.0;
  const RUNS_STEPPER_HEIGHT: f32 = 34.0;
  const RUNS_STEP_WIDTH: f32 = 30.0;
  const TAB_STRIP_HEIGHT: f32 = 40.0;
  const TILE_BOX: f32 = 30.0;
  const TREE_INDENT: f32 = 22.0;

  const COL_BOM_QTY: f32 = 96.0;
  const COL_COST: f32 = 140.0;
  const COL_PRICE: f32 = 120.0;
  const COL_QTY: f32 = 120.0;

  struct MaterialLine {
    building: bool,
    cost: f64,
    depth: usize,
    qty: i64,
    site: Option<i64>,
    unit: f64,
  }

  pub(super) fn body(planner: &Planner) -> Element<'_, Message> {
    let product = planner
      .product()
      .filter(|product| planner.data().recipe(*product).is_some());

    let left_content: Element<'_, Message> = match product {
      Some(product) => left_pane(
        planner,
        product,
        planner.data().recipe(product).expect("recipe checked"),
      ),
      None => empty_left(planner),
    };

    let left = scrollable(left_content)
      .id(MATERIAL_PLAN_SCROLL_ID)
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill);

    Row::with_children(vec![
      container(left).width(Length::Fill).height(Length::Fill).into(),
      pane_handle(Message::PaneDragStart),
      right_pane(planner, product),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
  }

  pub(super) fn loading<'a>() -> Element<'a, Message> {
    centered(
      text(t!("industry.planner.loading"))
        .font(typography::body::REGULAR)
        .size(typography::size::LG)
        .style(typography::colored(color::text::tertiary())),
    )
  }

  fn left_pane<'a>(planner: &'a Planner, product: i64, recipe: &'a Recipe) -> Element<'a, Message> {
    let has_plan = planner.has_plan();
    let merged = planner.merged_build_order();
    let steps = merged.len().max(1);

    let mut children: Vec<Element<'a, Message>> = vec![
      picker(planner),
      section_label(
        &t!("industry.tab.blueprints"),
        (steps > 1).then(|| t!("industry.planner.section_steps", count => steps).into_owned()),
      ),
      blueprint_card(planner, product, recipe, None),
    ];

    for job in merged.iter().filter(|job| !job.is_root).rev() {
      children.push(sub_blueprint_card(planner, job));
    }

    let me_hint = if recipe.is_reaction {
      t!("industry.planner.reaction_inputs").into_owned()
    } else {
      t!("industry.planner.me_applied", me => planner.settings_for(product).me).into_owned()
    };
    children.push(material_plan_header(
      planner,
      t!("industry.planner.material_plan_hint", me_hint => me_hint).into_owned(),
    ));
    children.push(material_plan(planner, recipe));

    if has_plan {
      children.push(bill_of_materials(planner));
      children.push(needed_blueprints(planner));
      children.push(build_order(planner));
    }

    Column::with_children(children)
      .spacing(spacing::SPACE_3)
      .padding(PANE_PADDING)
      .width(Length::Fill)
      .into()
  }

  fn picker(planner: &Planner) -> Element<'_, Message> {
    let active = planner.picker_open() || !planner.search().is_empty();
    let glyph_color = if active {
      color::accent::PLASMA
    } else {
      color::text::secondary()
    };

    let search = TextInput::new(planner.search_placeholder(), planner.search(), Message::SearchChanged)
      .input_id(crate::features::shell::focus_search::industry_search_id())
      .leading_icon(Icon::search().color(glyph_color))
      .background(color::surface::SUNKEN)
      .width(Length::Fill)
      .render();

    let toggle = button(
      Icon::chevron()
        .color(color::text::secondary())
        .size(16.0)
        .render::<Message>(),
    )
    .padding(spacing::SPACE_2_5)
    .on_press(Message::PickerToggled)
    .style(|_, _| button::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..button::Style::default()
    });

    let bar = Row::with_children(vec![container(search).width(Length::Fill).into(), toggle.into()])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center);

    let open = planner.picker_open() || !planner.search().is_empty();
    let popover = open.then(|| picker_results(planner));

    AnchoredDropdown::new(bar, popover)
      .on_dismiss(Message::PickerToggled)
      .into()
  }

  fn picker_results(planner: &Planner) -> Element<'_, Message> {
    let data = planner.data();
    let query = planner.search().trim().to_lowercase();
    let category = planner.category();

    let chips: Vec<Element<'_, Message>> = std::iter::once(category_chip(
      t!("industry.category.all"),
      Category::Other,
      true,
      category,
    ))
    .chain(
      Category::PICKER
        .into_iter()
        .map(|cat| category_chip(cat.label(), cat, false, category)),
    )
    .collect();

    let mut matches: Vec<i64> = data
      .catalog
      .iter()
      .filter(|entry| {
        query.is_empty()
          || entry.name.to_lowercase().contains(&query)
          || entry.group_name.to_lowercase().contains(&query)
      })
      .filter(|entry| matches_category(entry.category, category))
      .map(|entry| entry.type_id)
      .collect();
    matches.truncate(PICKER_MAX_RESULTS);

    let header = picker_header(&query, matches.len());
    let list = picker_list(planner, matches, &query);

    let panel = Column::with_children(vec![
      Row::with_children(chips).spacing(spacing::SPACE_2).into(),
      rule::horizontal(),
      micro_label(&header),
      container(list).width(Length::Fill).height(Length::Fixed(320.0)).into(),
    ])
    .spacing(spacing::SPACE_2)
    .padding(spacing::SPACE_3)
    .width(Length::Fill);

    container(panel)
      .width(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::rule_strong(),
          radius: radius::CARD.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn picker_header(query: &str, match_count: usize) -> String {
    if query.is_empty() && match_count == 0 {
      return t!("industry.planner.your_blueprints").into_owned();
    }
    t!(
      "industry.planner.results",
      count => match_count,
      plural => if match_count == 1 { "" } else { "s" }
    )
    .into_owned()
  }

  fn picker_list<'a>(planner: &'a Planner, matches: Vec<i64>, query: &str) -> Element<'a, Message> {
    if !matches.is_empty() {
      return picker_virtual_list(planner, matches);
    }
    let source = if query.is_empty() { planner.recent() } else { &[] };
    if source.is_empty() {
      return picker_empty(planner, query);
    }
    let rows: Vec<Element<'_, Message>> = source.iter().map(|&id| picker_row(planner, id)).collect();
    Column::with_children(rows).width(Length::Fill).into()
  }

  fn picker_empty<'a>(planner: &Planner, query: &str) -> Element<'a, Message> {
    let message = if query.is_empty() {
      t!("industry.planner.no_products").into_owned()
    } else {
      t!("industry.planner.no_products_query", query => planner.search().trim()).into_owned()
    };
    centered(
      text(message)
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::tertiary())),
    )
  }

  fn picker_virtual_list(planner: &Planner, matches: Vec<i64>) -> Element<'_, Message> {
    let offset = planner.picker_scroll_offset();
    virtual_list::responsive_window(move |height| {
      let config = VirtualListConfig::new(matches.len(), ESTIMATED_PICKER_ROW)
        .viewport_height(height)
        .scroll_offset(offset);
      let windowed = VirtualList::new(config, |index| picker_row(planner, matches[index])).view();
      scrollable(windowed)
        .style(crate::ui::style::control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fill)
        .on_scroll(|viewport| Message::PickerScrolled {
          absolute: viewport.absolute_offset().y,
        })
        .into()
    })
  }

  fn picker_row(planner: &Planner, type_id: i64) -> Element<'_, Message> {
    let data = planner.data();
    let recipe = data.recipe(type_id);
    let is_reaction = recipe.is_some_and(|recipe| recipe.is_reaction);

    let title = text(data.name(type_id))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY));

    let subtitle = text(t!("industry.planner.isk", value => fmt_isk(data.price(type_id))))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::tertiary()));

    let details = Column::with_children(vec![title.into(), subtitle.into()])
      .spacing(spacing::UNIT)
      .width(Length::Fill);

    let badges = Row::with_children(vec![activity_badge(is_reaction), owned_badge(planner, type_id)])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center);

    let row = Row::with_children(vec![type_tile(data.type_icon(type_id)), details.into(), badges.into()])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .width(Length::Fill);

    button(row)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_2,
        right: spacing::SPACE_2,
      })
      .on_press(Message::ProductPicked(type_id))
      .style(picker_row_style)
      .into()
  }

  fn category_chip<'a>(
    label: impl Into<String>,
    category: Category,
    is_all: bool,
    active: Category,
  ) -> Element<'a, Message> {
    let on = if is_all {
      active == Category::Other
    } else {
      active == category
    };
    let fill = if on {
      color::accent::PLASMA
    } else {
      color::text::secondary()
    };
    let target = if is_all { Category::Other } else { category };
    button(
      text(label.into())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(fill)),
    )
    .padding(Padding {
      top: spacing::UNIT,
      bottom: spacing::UNIT,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
    })
    .on_press(Message::CategorySelected(target))
    .style(move |_, _| button::Style {
      background: on.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.14))),
      border: Border {
        color: if on { color::accent::PLASMA } else { color::rule() },
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      text_color: fill,
      ..button::Style::default()
    })
    .into()
  }

  fn blueprint_card<'a>(
    planner: &'a Planner,
    type_id: i64,
    recipe: &'a Recipe,
    job: Option<&MergedBuildJob>,
  ) -> Element<'a, Message> {
    let config = planner.settings_for(type_id);
    let is_reaction = recipe.is_reaction;
    let runs = job.map(|job| job.runs).unwrap_or_else(|| planner.runs());

    let header = blueprint_header(planner, type_id, is_reaction, job);

    let mut center: Vec<Element<'a, Message>> = Vec::new();
    if !is_reaction {
      center.push(efficiency_slider(
        &t!("industry.planner.material_efficiency"),
        config.me,
        super::ME_MAX,
        type_id,
        true,
      ));
      center.push(efficiency_slider(
        &t!("industry.planner.time_efficiency"),
        config.te,
        super::TE_MAX,
        type_id,
        false,
      ));
    }

    let controls = Row::with_children(vec![
      runs_control(runs, planner.runs_input(), job.is_some(), is_reaction),
      Space::new().width(Length::Fill).into(),
      Row::with_children(center)
        .spacing(spacing::SPACE_6)
        .align_y(Vertical::Top)
        .into(),
      Space::new().width(Length::Fill).into(),
      facility_control(planner, type_id, is_reaction),
    ])
    .spacing(spacing::SPACE_6)
    .align_y(Vertical::Top)
    .width(Length::Fill);

    let mut body: Vec<Element<'a, Message>> = vec![header, controls.into()];

    if let Some(job) = job {
      body.push(rule::horizontal());
      body.push(build_vs_buy(planner, type_id, recipe, job));
    }

    container(Column::with_children(body).spacing(spacing::SPACE_3).padding(Padding {
      top: spacing::SPACE_3_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_3_5,
      right: spacing::SPACE_3_5,
    }))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }

  fn sub_blueprint_card<'a>(planner: &'a Planner, job: &MergedBuildJob) -> Element<'a, Message> {
    let Some(recipe) = planner.data().recipe(job.type_id) else {
      return Space::new().into();
    };
    blueprint_card(planner, job.type_id, recipe, Some(job))
  }

  fn blueprint_header<'a>(
    planner: &'a Planner,
    type_id: i64,
    is_reaction: bool,
    job: Option<&MergedBuildJob>,
  ) -> Element<'a, Message> {
    let data = planner.data();
    let mut badges = Row::with_children(vec![
      text(data.name(type_id))
        .font(typography::body::MEDIUM)
        .size(typography::size::LG)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      activity_badge(is_reaction),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);
    badges = if job.is_some() {
      badges.push(badge(t!("industry.planner.building"), Some(color::status::WARNING)))
    } else {
      badges.push(owned_badge(planner, type_id))
    };

    let subtitle = match job {
      Some(job) => t!(
        "industry.planner.builds_subtitle",
        output => fmt_num(data.recipe(type_id).map(|r| r.output_per_run).unwrap_or(1) * job.runs),
        needed => fmt_num(job.needed_qty),
        feeds => merged_feeds_line(data, job)
      )
      .into_owned(),
      None => t!("industry.planner.isk_each", price => fmt_isk(data.price(type_id))).into_owned(),
    };

    let details = Column::with_children(vec![
      badges.into(),
      text(subtitle)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

    let mut row = Row::with_children(vec![type_tile(data.type_icon(type_id)), details.into()])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .width(Length::Fill);

    if job.is_some() {
      row = row.push(
        button(
          Icon::close()
            .color(color::text::tertiary())
            .size(14.0)
            .render::<Message>(),
        )
        .padding(spacing::UNIT)
        .on_press(Message::NodeCollapsed {
          type_id,
        })
        .style(|_, _| button::Style::default()),
      );
    }

    row.into()
  }

  fn runs_control<'a>(runs: i64, runs_text: &'a str, locked: bool, is_reaction: bool) -> Element<'a, Message> {
    let mut label = if is_reaction {
      t!("industry.planner.runs_cycles").into_owned()
    } else {
      t!("industry.planner.runs_runs").into_owned()
    };
    if locked {
      label.push_str(&t!("industry.planner.runs_locked"));
    }

    let value: Element<'a, Message> = if locked {
      Row::with_children(vec![
        text(fmt_num(runs))
          .font(typography::mono::MEDIUM)
          .size(typography::size::LG)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
        text(t!("industry.planner.for_job"))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into()
    } else {
      runs_stepper(runs, runs_text)
    };

    Column::with_children(vec![micro_label(&label), value])
      .spacing(spacing::SPACE_2)
      .into()
  }

  fn runs_stepper<'a>(runs: i64, runs_text: &'a str) -> Element<'a, Message> {
    let field = text_input("1", runs_text)
      .on_input(Message::RunsInputChanged)
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .align_x(Horizontal::Center)
      .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: spacing::UNIT,
        right: spacing::UNIT,
      })
      .width(Length::Fixed(RUNS_FIELD_WIDTH))
      .style(text_input_inner_style());

    let segments = Row::with_children(vec![
      segment_button("\u{2212}", (runs > 1).then(|| Message::RunsChanged(runs - 1)), false),
      divider(),
      container(field)
        .height(Length::Fixed(RUNS_STEPPER_HEIGHT))
        .align_y(Vertical::Center)
        .into(),
      divider(),
      segment_button("+", Some(Message::RunsChanged(runs + 1)), true),
    ])
    .align_y(Vertical::Center);

    container(segments)
      .height(Length::Fixed(RUNS_STEPPER_HEIGHT))
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          color: color::rule_strong(),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn divider<'a>() -> Element<'a, Message> {
    container(
      Space::new()
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(RUNS_STEPPER_HEIGHT)),
    )
    .style(|_| container::Style {
      background: Some(Background::Color(color::rule())),
      ..container::Style::default()
    })
    .into()
  }

  fn segment_button<'a>(glyph: &str, on_press: Option<Message>, right: bool) -> Element<'a, Message> {
    let mut control = button(
      container(
        text(glyph.to_owned())
          .font(typography::mono::REGULAR)
          .size(typography::size::LG)
          .style(typography::colored(color::text::secondary())),
      )
      .width(Length::Fixed(RUNS_STEP_WIDTH))
      .height(Length::Fixed(RUNS_STEPPER_HEIGHT))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
    )
    .padding(Padding::ZERO);
    if let Some(message) = on_press {
      control = control.on_press(message);
    }

    let radius = if right {
      iced::border::Radius::default().right(radius::CONTROL)
    } else {
      iced::border::Radius::default().left(radius::CONTROL)
    };
    control
      .style(move |_, _| button::Style {
        background: Some(Background::Color(iced::Color::TRANSPARENT)),
        border: Border {
          radius,
          ..Border::default()
        },
        text_color: color::text::secondary(),
        ..button::Style::default()
      })
      .into()
  }

  fn efficiency_slider<'a>(label: &str, value: i64, max: i64, type_id: i64, material: bool) -> Element<'a, Message> {
    let prefix = if material {
      t!("industry.planner.material_eff_short")
    } else {
      t!("industry.planner.time_eff_short")
    };
    let handle = move |next: f64| {
      let next = next.round() as i64;
      if material {
        Message::MaterialEfficiencyChanged {
          me: next,
          type_id,
        }
      } else {
        Message::TimeEfficiencyChanged {
          te: next,
          type_id,
        }
      }
    };
    let step: f64 = if material { 1.0 } else { 2.0 };
    let control = slider(0.0..=max as f64, value as f64, handle)
      .step(step)
      .height(6.0)
      .style(crate::ui::style::control::slider_track)
      .width(Length::Fixed(120.0));

    Column::with_children(vec![
      micro_label(label),
      Row::with_children(vec![
        control.into(),
        text(t!("industry.planner.efficiency_readout", prefix => prefix, value => value))
          .font(typography::mono::MEDIUM)
          .size(typography::size::MD)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into(),
    ])
    .spacing(spacing::SPACE_2)
    .into()
  }

  fn facility_control<'a>(planner: &'a Planner, type_id: i64, is_reaction: bool) -> Element<'a, Message> {
    let selected = planner.selected_facility(type_id, is_reaction);

    let placeholder: &'a str = if planner.data().facilities.is_empty() {
      "No facilities available"
    } else {
      "Select a facility\u{2026}"
    };

    let trigger = FacilityCombobox::new()
      .placeholder(placeholder)
      .selection(selected.map(|facility| facility_ref(facility, is_reaction)))
      .on_toggle(Message::FacilityPickerToggled {
        type_id,
      })
      .trigger();

    let open = planner
      .facility_picker()
      .filter(|state| state.type_id == type_id)
      .map(|state| facility_picker_popover(planner, state, type_id, is_reaction));

    let dropdown = AnchoredDropdown::new(trigger, open).on_dismiss(Message::FacilityPickerToggled {
      type_id,
    });

    Column::with_children(vec![micro_label(&t!("industry.planner.build_at")), dropdown.into()])
      .spacing(spacing::SPACE_2)
      .width(Length::Fixed(FACILITY_PICKER_WIDTH))
      .into()
  }

  fn facility_picker_popover<'a>(
    planner: &'a Planner,
    state: &'a super::FacilityPickerState,
    type_id: i64,
    is_reaction: bool,
  ) -> Element<'a, Message> {
    let selected = planner
      .selected_facility(type_id, is_reaction)
      .map(|f| facility_ref(f, is_reaction));

    let facilities: Vec<FacilityRef> = state.results.iter().map(|f| facility_ref(f, is_reaction)).collect();

    let popover = FacilityCombobox::new()
      .query(state.query.as_str())
      .results(facilities)
      .on_input(move |value| Message::FacilitySearchChanged {
        query: value,
        type_id,
      })
      .on_pick(move |facility: FacilityRef| Message::FacilitySelected {
        facility_structure: facility.id,
        pin: pin_for(&facility),
        solar_system_id: facility.solar_system_id,
        type_id,
      })
      .width(Length::Fill)
      .searching(state.searching)
      .selection(selected)
      .popover();

    container(popover)
      .style(|_| container::Style {
        shadow: crate::ui::style::shadow::CARD,
        ..container::Style::default()
      })
      .into()
  }

  fn pin_for(facility: &FacilityRef) -> Option<super::PinnedStructure> {
    (facility.id >= MIN_STRUCTURE_ID).then(|| super::PinnedStructure {
      id: facility.id,
      name: facility.name.clone(),
      solar_system_id: facility.solar_system_id,
      type_id: facility.type_id,
    })
  }

  fn facility_ref(facility: &PlannerFacility, is_reaction: bool) -> FacilityRef {
    facility.to_ref(is_reaction)
  }

  fn build_vs_buy<'a>(
    planner: &'a Planner,
    type_id: i64,
    recipe: &'a Recipe,
    job: &MergedBuildJob,
  ) -> Element<'a, Message> {
    let data = planner.data();
    let config = planner.settings_for(type_id);
    let rig_factors = planner.rig_factors_for(config.facility_structure);
    let material_cost: f64 = recipe
      .materials
      .iter()
      .map(|m| {
        eff_qty(m.base_qty, job.runs, config.me, recipe.is_reaction, rig_factors.me) as f64 * data.price(m.type_id)
      })
      .sum();
    let eiv = super::estimated_item_value(data, recipe, job.runs);
    let fee = super::install_fee(eiv, planner.cost_index(type_id).unwrap_or(0.0), rig_factors.fee);
    let build_cost = material_cost + fee;
    let buy_cost = job.needed_qty as f64 * data.price(type_id);
    let savings = buy_cost - build_cost;
    let build_time = node_build_time(recipe, job.runs, config.te, rig_factors.te);

    let (savings_label, savings_color) = if savings >= 0.0 {
      ("Saved", color::status::ONLINE)
    } else {
      ("Costs extra", color::status::DANGER)
    };

    Row::with_children(vec![
      metric("Build cost", &fmt_isk(build_cost), color::text::PRIMARY),
      metric("vs. buying", &fmt_isk(buy_cost), color::text::secondary()),
      metric(
        savings_label,
        &format!(
          "{}{}",
          if savings >= 0.0 { "+" } else { "\u{2212}" },
          fmt_isk(savings.abs())
        ),
        savings_color,
      ),
      metric(
        "Build time",
        &fmt_duration_coarse(build_time as i64),
        color::text::secondary(),
      ),
    ])
    .spacing(spacing::SPACE_6)
    .into()
  }

  fn material_plan_header(planner: &Planner, hint: String) -> Element<'_, Message> {
    let label = section_label(&t!("industry.planner.material_plan"), Some(hint));
    if !planner.has_buildable_inputs() {
      return label;
    }
    Row::with_children(vec![
      container(label).width(Length::Fill).into(),
      break_down_all_button(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
  }

  fn break_down_all_button<'a>() -> Element<'a, Message> {
    let inner = Row::with_children(vec![
      Icon::flask()
        .color(color::status::WARNING)
        .size(13.0)
        .render::<Message>(),
      text(t!("industry.planner.break_down_all"))
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(typography::colored(color::status::WARNING))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

    button(inner)
      .padding(Padding {
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      })
      .on_press(Message::BreakDownAll)
      .style(|_, _| button::Style {
        background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.1))),
        border: Border {
          color: color::with_alpha(color::status::WARNING, 0.34),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: color::status::WARNING,
        ..button::Style::default()
      })
      .into()
  }

  fn material_plan<'a>(planner: &'a Planner, recipe: &'a Recipe) -> Element<'a, Message> {
    let site = planner
      .product()
      .and_then(|product| planner.settings_for(product).facility_structure);
    let mut acc = MaterialRowsAcc {
      out: vec![grid_header()],
      seen: std::collections::BTreeSet::new(),
      total: 0.0,
    };
    material_rows(planner, recipe, planner.runs(), site, 0, &mut acc);

    acc.out.push(footer_row(
      &t!("industry.planner.material_cost"),
      &fmt_isk_full(acc.total),
    ));

    container(Column::with_children(acc.out).width(Length::Fill))
      .width(Length::Fill)
      .style(bordered_table)
      .into()
  }

  struct MaterialRowsAcc<'a> {
    out: Vec<Element<'a, Message>>,
    seen: std::collections::BTreeSet<i64>,
    total: f64,
  }

  fn material_rows<'a>(
    planner: &'a Planner,
    recipe: &'a Recipe,
    runs: i64,
    site: Option<i64>,
    depth: usize,
    acc: &mut MaterialRowsAcc<'a>,
  ) {
    let data = planner.data();
    let rig_me_factor = planner.rig_factors_for(site).me;
    for material in &recipe.materials {
      let qty = eff_qty(
        material.base_qty,
        runs,
        runs_me(planner, recipe),
        recipe.is_reaction,
        rig_me_factor,
      );
      let unit = data.price(material.type_id);
      let cost = qty as f64 * unit;
      let child = planner.is_built(material.type_id);
      if depth == 0 {
        acc.total += cost;
      }

      acc.out.push(material_row(
        planner,
        material.type_id,
        MaterialLine {
          building: child,
          cost,
          depth,
          qty,
          site,
          unit,
        },
      ));

      if child
        && !planner.is_row_collapsed(material.type_id)
        && acc.seen.insert(material.type_id)
        && let Some(child_recipe) = data.recipe(material.type_id)
      {
        let child_runs = runs_for(qty, child_recipe.output_per_run);
        let child_site = planner.settings_for(material.type_id).facility_structure;
        material_rows(planner, child_recipe, child_runs, child_site, depth + 1, acc);
        acc.seen.remove(&material.type_id);
      }
    }
  }

  fn runs_me(planner: &Planner, recipe: &Recipe) -> i64 {
    if recipe.is_reaction {
      return 0;
    }
    planner
      .product_for_blueprint(recipe.blueprint_type_id)
      .map(|product| planner.settings_for(product).me)
      .unwrap_or(super::DEFAULT_ME)
  }

  fn breakdown_button<'a>(type_id: i64) -> Element<'a, Message> {
    let inner = Row::with_children(vec![
      Icon::flask()
        .color(color::status::WARNING)
        .size(11.0)
        .render::<Message>(),
      text(t!("industry.planner.breakdown"))
        .font(typography::body::MEDIUM)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::status::WARNING))
        .into(),
    ])
    .spacing(spacing::UNIT + 1.0)
    .align_y(Vertical::Center);

    button(inner)
      .padding(Padding {
        top: spacing::UNIT,
        bottom: spacing::UNIT,
        left: spacing::SPACE_2,
        right: spacing::SPACE_2,
      })
      .on_press(Message::NodeBrokenDown {
        type_id,
      })
      .style(|_, _| button::Style {
        background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.1))),
        border: Border {
          color: color::with_alpha(color::status::WARNING, 0.3),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: color::status::WARNING,
        ..button::Style::default()
      })
      .into()
  }

  fn use_stock_button<'a>(site: i64, type_id: i64) -> Element<'a, Message> {
    let inner = Row::with_children(vec![
      Icon::assets()
        .color(color::status::ONLINE)
        .size(11.0)
        .render::<Message>(),
      text(t!("industry.planner.use_stock"))
        .font(typography::body::MEDIUM)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::status::ONLINE))
        .into(),
    ])
    .spacing(spacing::UNIT + 1.0)
    .align_y(Vertical::Center);

    button(inner)
      .padding(Padding {
        top: spacing::UNIT,
        bottom: spacing::UNIT,
        left: spacing::SPACE_2,
        right: spacing::SPACE_2,
      })
      .on_press(Message::StockSelectionToggled {
        site,
        type_id,
      })
      .style(|_, _| button::Style {
        background: Some(Background::Color(color::with_alpha(color::status::ONLINE, 0.1))),
        border: Border {
          color: color::with_alpha(color::status::ONLINE, 0.3),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: color::status::ONLINE,
        ..button::Style::default()
      })
      .into()
  }

  fn stock_chip<'a>(site: i64, type_id: i64) -> Element<'a, Message> {
    let inner = Row::with_children(vec![
      Icon::check().color(color::status::ONLINE).size(9.0).render::<Message>(),
      text(t!("industry.planner.stock"))
        .font(typography::mono::MEDIUM)
        .size(typography::size::XS)
        .style(typography::colored(color::status::ONLINE))
        .into(),
    ])
    .spacing(spacing::UNIT)
    .align_y(Vertical::Center);

    button(inner)
      .padding(Padding {
        top: spacing::UNIT,
        bottom: spacing::UNIT,
        left: spacing::SPACE_2,
        right: spacing::SPACE_2,
      })
      .on_press(Message::StockSelectionToggled {
        site,
        type_id,
      })
      .style(|_, _| button::Style {
        background: Some(Background::Color(color::with_alpha(color::status::ONLINE, 0.18))),
        border: Border {
          color: color::with_alpha(color::status::ONLINE, 0.44),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: color::status::ONLINE,
        ..button::Style::default()
      })
      .into()
  }

  fn qty_split_cell<'a>(qty: i64, drawn: i64, remaining: i64, building: bool) -> Element<'a, Message> {
    let total = container(
      text(fmt_num(qty))
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY)),
    )
    .width(Length::Fill)
    .align_x(Horizontal::Right);

    let mut column = Column::with_children(vec![total.into()]).spacing(spacing::UNIT);

    if drawn > 0 {
      let mut tokens: Vec<Element<'a, Message>> = vec![
        text(t!("industry.planner.qty.stock", count => fmt_num(drawn)))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::status::ONLINE))
          .into(),
      ];
      if remaining > 0 {
        let (label, tint) = if building {
          (
            t!("industry.planner.qty.build", count => fmt_num(remaining)).into_owned(),
            color::status::WARNING,
          )
        } else {
          (
            t!("industry.planner.qty.buy", count => fmt_num(remaining)).into_owned(),
            color::text::tertiary(),
          )
        };
        tokens.push(
          text("\u{00B7}")
            .font(typography::mono::REGULAR)
            .size(typography::size::XS)
            .style(typography::colored(color::text::tertiary()))
            .into(),
        );
        tokens.push(
          text(label)
            .font(typography::mono::REGULAR)
            .size(typography::size::XS)
            .style(typography::colored(tint))
            .into(),
        );
      }
      column = column.push(
        container(
          Row::with_children(tokens)
            .spacing(spacing::UNIT + 1.0)
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Right),
      );
    }

    container(column).width(Length::Fixed(COL_QTY)).into()
  }

  struct StockSplit {
    can_use: bool,
    drawn: i64,
    remaining: i64,
    site: Option<i64>,
    using: bool,
  }

  fn stock_split(planner: &Planner, type_id: i64, qty: i64, site: Option<i64>) -> StockSplit {
    let using = site.is_some_and(|site| planner.is_stock_selected(site, type_id));
    let drawn = match site {
      Some(site) if using => planner.on_hand_at(site, type_id).min(qty),
      _ => 0,
    };
    StockSplit {
      can_use: !using && site.is_some_and(|site| planner.remaining_pool(site, type_id) > 0),
      drawn,
      remaining: (qty - drawn).max(0),
      site,
      using,
    }
  }

  fn stock_affordance<'a>(type_id: i64, split: &StockSplit) -> Option<Element<'a, Message>> {
    let site = split.site?;
    if split.using {
      Some(stock_chip(site, type_id))
    } else if split.can_use {
      Some(use_stock_button(site, type_id))
    } else {
      None
    }
  }

  fn collapse_chevron<'a>(type_id: i64, collapsed: bool) -> Element<'a, Message> {
    let glyph = if collapsed {
      Icon::chevron_right()
    } else {
      Icon::chevron()
    };

    button(glyph.color(color::text::secondary()).size(12.0).render::<Message>())
      .padding(Padding::ZERO)
      .on_press(Message::RowCollapseToggled {
        type_id,
      })
      .style(|_, _| button::Style::default())
      .into()
  }

  fn material_row<'a>(planner: &'a Planner, type_id: i64, line: MaterialLine) -> Element<'a, Message> {
    let MaterialLine {
      building,
      cost,
      depth,
      qty,
      site,
      unit,
    } = line;
    let data = planner.data();
    let buildable = data.recipe(type_id).is_some();
    let split = stock_split(planner, type_id, qty, site);

    let mut name_row = Row::new().spacing(spacing::SPACE_2).align_y(Vertical::Center);
    if building {
      name_row = name_row.push(collapse_chevron(type_id, planner.is_row_collapsed(type_id)));
    }
    name_row = name_row.push(type_tile(data.type_icon(type_id)));
    name_row = name_row.push(
      text(data.name(type_id))
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY)),
    );
    if building {
      name_row = name_row.push(badge(t!("industry.planner.building"), Some(color::status::WARNING)));
    }
    if let Some(affordance) = stock_affordance(type_id, &split) {
      name_row = name_row.push(affordance);
    }
    if !building && buildable {
      name_row = name_row.push(breakdown_button(type_id));
    }

    let name_cell = container(name_row).padding(Padding {
      left: depth as f32 * TREE_INDENT,
      ..Padding::ZERO
    });

    let grid = Row::with_children(vec![
      container(name_cell).width(Length::Fill).into(),
      qty_split_cell(qty, split.drawn, split.remaining, building),
      grid_value(&fmt_price(unit), COL_PRICE, color::text::secondary()),
      grid_value(
        &fmt_isk(cost),
        COL_COST,
        if building {
          color::status::WARNING
        } else {
          color::text::PRIMARY
        },
      ),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

    let background = if split.using {
      color::with_alpha(color::status::ONLINE, 0.07)
    } else if building {
      color::with_alpha(color::status::WARNING, 0.07)
    } else if depth > 0 {
      color::with_alpha(color::surface::SUNKEN, 0.45)
    } else {
      iced::Color::TRANSPARENT
    };

    let styled = container(grid)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2_5,
        bottom: spacing::SPACE_2_5,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      })
      .style(move |_| container::Style {
        background: Some(Background::Color(background)),
        border: Border {
          color: color::rule(),
          radius: 0.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      });

    let mut area = mouse_area(styled).on_right_press(Message::MaterialRightPressed {
      type_id,
    });
    if buildable && !building {
      area = area.on_press(Message::NodeBrokenDown {
        type_id,
      });
    }
    area.into()
  }

  fn bill_of_materials<'a>(planner: &'a Planner) -> Element<'a, Message> {
    let data = planner.data();
    let allocation = planner.stock_allocation();
    let mut totals = planner.raw_totals().to_vec();
    totals.sort_by(|a, b| (b.qty as f64 * data.price(b.type_id)).total_cmp(&(a.qty as f64 * data.price(a.type_id))));

    let buy_cost: f64 = totals
      .iter()
      .map(|total| to_buy_qty(&allocation, total) as f64 * data.price(total.type_id))
      .sum();
    let inventory_value: f64 = totals
      .iter()
      .map(|total| drawn_qty(&allocation, total) as f64 * data.price(total.type_id))
      .sum();
    let stocked = totals.iter().filter(|total| drawn_qty(&allocation, total) > 0).count();

    let mut rows: Vec<Element<'a, Message>> = vec![bom_grid_header()];
    for total in &totals {
      rows.push(bom_row(planner, &allocation, total));
    }
    if inventory_value > 0.0 {
      rows.push(footer_row(
        &t!("industry.planner.covered_from_inventory"),
        &fmt_isk_full(inventory_value),
      ));
    }
    let footer_label = if inventory_value > 0.0 {
      t!("industry.planner.cost_to_buy")
    } else {
      t!("industry.planner.acquisition_cost")
    };
    rows.push(footer_row(&footer_label, &fmt_isk_full(buy_cost)));

    let hint = if stocked > 0 {
      t!(
        "industry.planner.bom_hint_stocked",
        count => totals.len(),
        stocked => stocked
      )
      .into_owned()
    } else {
      t!("industry.planner.bom_hint", count => totals.len()).into_owned()
    };

    Column::with_children(vec![
      section_label(&t!("industry.planner.bom_section"), Some(hint)),
      container(Column::with_children(rows).width(Length::Fill))
        .width(Length::Fill)
        .style(bordered_table)
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .into()
  }

  fn drawn_qty(allocation: &super::StockAllocation, total: &super::RawTotal) -> i64 {
    allocation.drawn_for_type(total.type_id).min(total.qty).max(0)
  }

  fn to_buy_qty(allocation: &super::StockAllocation, total: &super::RawTotal) -> i64 {
    (total.qty - drawn_qty(allocation, total)).max(0)
  }

  fn bom_row<'a>(
    planner: &'a Planner,
    allocation: &super::StockAllocation,
    total: &super::RawTotal,
  ) -> Element<'a, Message> {
    let data = planner.data();
    let drawn = drawn_qty(allocation, total);
    let to_buy = to_buy_qty(allocation, total);
    let unit = data.price(total.type_id);
    let buy_cost = to_buy as f64 * unit;
    let stocked = drawn > 0;

    let name = container(
      Row::with_children(vec![
        type_tile(data.type_icon(total.type_id)),
        text(data.name(total.type_id))
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
    )
    .width(Length::Fill);

    let from_stock = if stocked {
      grid_value(&fmt_num(drawn), COL_BOM_QTY, color::status::ONLINE)
    } else {
      grid_value("\u{2014}", COL_BOM_QTY, color::text::tertiary())
    };
    let to_buy_cell = if to_buy > 0 {
      grid_value(&fmt_num(to_buy), COL_BOM_QTY, color::text::PRIMARY)
    } else {
      grid_value("0", COL_BOM_QTY, color::status::ONLINE)
    };
    let subtotal = if to_buy > 0 {
      grid_value(&fmt_isk(buy_cost), COL_COST, color::text::PRIMARY)
    } else {
      grid_value("\u{2014}", COL_COST, color::text::tertiary())
    };

    let row = Row::with_children(vec![
      name.into(),
      grid_value(&fmt_num(total.qty), COL_BOM_QTY, color::text::PRIMARY),
      from_stock,
      to_buy_cell,
      subtotal,
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

    let background = if stocked {
      color::with_alpha(color::status::ONLINE, 0.06)
    } else {
      iced::Color::TRANSPARENT
    };

    container(row)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2_5,
        bottom: spacing::SPACE_2_5,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      })
      .style(move |_| container::Style {
        background: Some(Background::Color(background)),
        border: Border {
          color: color::rule(),
          radius: 0.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn bom_grid_header<'a>() -> Element<'a, Message> {
    let head = |label: &str, width: Option<f32>| -> Element<'a, Message> {
      let content = text(label.to_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()));
      match width {
        Some(width) => container(container(content).width(Length::Fill).align_x(Horizontal::Right))
          .width(Length::Fixed(width))
          .into(),
        None => container(content).width(Length::Fill).into(),
      }
    };

    container(
      Row::with_children(vec![
        head(&t!("industry.planner.grid_material"), None),
        head(&t!("industry.planner.bom_total"), Some(COL_BOM_QTY)),
        head(&t!("industry.planner.bom_from_stock"), Some(COL_BOM_QTY)),
        head(&t!("industry.planner.bom_to_buy"), Some(COL_BOM_QTY)),
        head(&t!("industry.planner.grid_subtotal"), Some(COL_COST)),
      ])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
  }

  fn build_order<'a>(planner: &'a Planner) -> Element<'a, Message> {
    let jobs = planner.merged_build_order();
    let count = jobs.len();
    let (assigned, total_segments) = planner.order_assignment();

    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for (index, job) in jobs.iter().enumerate() {
      rows.push(build_order_row(planner, index, job));
    }

    let mut hint = t!(
      "industry.planner.build_order_jobs",
      count => count,
      plural => if count == 1 { "" } else { "s" }
    )
    .into_owned();
    if total_segments > count {
      hint.push_str(&t!("industry.planner.build_order_runs", total => total_segments));
    }
    hint.push_str(&t!(
      "industry.planner.build_order_assigned",
      assigned => assigned,
      total => total_segments
    ));

    Column::with_children(vec![
      section_label(&t!("industry.planner.build_order"), Some(hint)),
      container(Column::with_children(rows).width(Length::Fill))
        .width(Length::Fill)
        .style(bordered_table)
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .into()
  }

  fn build_order_row<'a>(planner: &'a Planner, index: usize, job: &MergedBuildJob) -> Element<'a, Message> {
    let data = planner.data();
    let is_final = job.is_root;
    let recipe = recipe_for(data, job.type_id);
    let is_reaction = job.node.is_reaction;
    let segments = planner.segments_for(job.type_id);
    let split = segments.len() > 1;
    let time: f64 = segments
      .iter()
      .map(|segment| {
        segment_build_time(
          &recipe,
          segment.runs,
          job.node.te,
          job.node.rig_te_factor,
          planner.segment_assignment(segment),
        )
      })
      .sum();

    let mut name_row: Vec<Element<'a, Message>> = vec![
      text(data.name(job.type_id))
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
      activity_badge(is_reaction),
    ];
    if split {
      name_row.push(badge(
        t!("industry.planner.way", count => segments.len()),
        Some(color::accent::PLASMA),
      ));
    }

    let header = Row::with_children(vec![
      text(format!("{:02}", index + 1))
        .font(typography::mono::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(if is_final {
          color::accent::PLASMA
        } else {
          color::text::secondary()
        }))
        .into(),
      type_tile(data.type_icon(job.type_id)),
      Column::with_children(vec![
        Row::with_children(name_row)
          .spacing(spacing::SPACE_2)
          .align_y(Vertical::Center)
          .into(),
        text(merged_feeds_line(data, job))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      ])
      .spacing(spacing::UNIT)
      .width(Length::Fill)
      .into(),
      assignment_slot(planner, job.type_id, &segments, split),
      runs_pill(job.runs, is_reaction, is_final),
      text(fmt_duration_coarse(time as i64))
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

    let type_id = job.type_id;
    let header_area = mouse_area(container(header).width(Length::Fill).padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    }))
    .on_right_press(Message::OrderJobRightPressed {
      type_id,
    });

    let mut rows: Vec<Element<'a, Message>> = vec![header_area.into()];
    if split {
      let ctx = SegmentRow {
        count: segments.len(),
        is_reaction,
        recipe: &recipe,
        te: job.node.te,
        type_id,
      };
      for (i, segment) in segments.iter().enumerate() {
        rows.push(segment_row(planner, &ctx, i, segment));
      }
    }

    container(Column::with_children(rows).width(Length::Fill))
      .width(Length::Fill)
      .style(move |_| container::Style {
        background: is_final.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.07))),
        border: Border {
          color: color::rule(),
          radius: 0.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn assignment_slot<'a>(
    planner: &'a Planner,
    type_id: i64,
    segments: &[PlanSegment],
    split: bool,
  ) -> Element<'a, Message> {
    if split {
      let pilots: std::collections::BTreeSet<i64> = segments.iter().filter_map(|segment| segment.pilot_id).collect();
      let lead = if pilots.is_empty() {
        t!("industry.planner.pilot_unassigned").into_owned()
      } else {
        t!(
          "industry.planner.pilots_assigned",
          count => pilots.len(),
          plural => if pilots.len() == 1 { "" } else { "s" }
        )
        .into_owned()
      };
      return Column::with_children(vec![
        text(lead)
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::secondary()))
          .into(),
        text(t!("industry.planner.split_below"))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      ])
      .spacing(spacing::UNIT)
      .align_x(Horizontal::Right)
      .width(Length::Fixed(ASSIGN_SLOT_WIDTH))
      .into();
    }

    pilot_slot(planner, type_id, 0, segments.first())
  }

  fn pilot_slot<'a>(
    planner: &'a Planner,
    type_id: i64,
    index: usize,
    segment: Option<&PlanSegment>,
  ) -> Element<'a, Message> {
    if !planner.assign_pilots() {
      return assign_disabled_hint();
    }
    pilot_picker(planner, type_id, index, segment)
  }

  fn assign_disabled_hint<'a>() -> Element<'a, Message> {
    container(
      text(t!("industry.planner.assign_disabled"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary())),
    )
    .width(Length::Fixed(ASSIGN_SLOT_WIDTH))
    .height(Length::Fixed(ASSIGN_SLOT_HEIGHT))
    .align_x(Horizontal::Left)
    .align_y(Vertical::Center)
    .padding(Padding {
      left: spacing::SPACE_2,
      right: spacing::SPACE_2,
      ..Padding::ZERO
    })
    .into()
  }

  fn pilot_picker<'a>(
    planner: &'a Planner,
    type_id: i64,
    index: usize,
    segment: Option<&PlanSegment>,
  ) -> Element<'a, Message> {
    let assigned = segment.and_then(|segment| planner.segment_assignment(segment));

    let trigger = pilot_trigger(type_id, index, assigned);
    let open = planner
      .pilot_picker_open(type_id, index)
      .then(|| pilot_popover(planner, type_id, index, segment));

    AnchoredDropdown::new(trigger, open)
      .popover_width(PILOT_PICKER_WIDTH)
      .on_dismiss(Message::OrderPilotPickerToggled {
        index,
        type_id,
      })
      .into()
  }

  fn pilot_trigger<'a>(
    type_id: i64,
    index: usize,
    assigned: Option<(&'a PlanPilot, Option<&'a PlanClone>)>,
  ) -> Element<'a, Message> {
    let body: Element<'a, Message> = match assigned {
      Some((pilot, clone)) => {
        let clone_label = clone
          .map(|clone| clone.name.clone())
          .unwrap_or_else(|| t!("industry.planner.clone_active").into_owned());
        Row::with_children(vec![
          avatar(
            pilot.id,
            &pilot.name,
            Length::Fixed(ASSIGN_SLOT_HEIGHT - 12.0),
            ASSIGN_SLOT_HEIGHT - 12.0,
            pilot.portrait.clone(),
          ),
          Column::with_children(vec![
            text(pilot.name.clone())
              .font(typography::body::MEDIUM)
              .size(typography::size::SM)
              .style(typography::colored(color::text::PRIMARY))
              .into(),
            text(clone_label)
              .font(typography::mono::REGULAR)
              .size(typography::size::XS)
              .style(typography::colored(color::text::tertiary()))
              .into(),
          ])
          .spacing(spacing::UNIT)
          .width(Length::Fill)
          .into(),
        ])
        .spacing(spacing::SPACE_2)
        .align_y(Vertical::Center)
        .width(Length::Fill)
        .into()
      }
      None => text(t!("industry.planner.assign_pilot"))
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .width(Length::Fill)
        .into(),
    };

    let solid = assigned.is_some();
    button(
      Row::with_children(vec![
        body,
        Icon::chevron()
          .color(color::text::secondary())
          .size(13.0)
          .render::<Message>(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .width(Length::Fill),
    )
    .width(Length::Fixed(ASSIGN_SLOT_WIDTH))
    .padding(Padding {
      left: spacing::SPACE_2,
      right: spacing::SPACE_2,
      ..Padding::ZERO
    })
    .on_press(Message::OrderPilotPickerToggled {
      index,
      type_id,
    })
    .style(move |_, status| {
      let active = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: solid.then_some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          color: if active {
            color::accent::PLASMA
          } else if solid {
            color::rule_strong()
          } else {
            color::rule()
          },
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      }
    })
    .into()
  }

  fn pilot_popover<'a>(
    planner: &'a Planner,
    type_id: i64,
    index: usize,
    segment: Option<&PlanSegment>,
  ) -> Element<'a, Message> {
    let assigned_pilot = segment.and_then(|segment| segment.pilot_id);
    let assigned_clone = segment.and_then(|segment| segment.clone_id);
    let expanded = planner
      .pilot_picker_expanded()
      .or(assigned_pilot)
      .or_else(|| planner.pilots().first().map(|pilot| pilot.id));

    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    if assigned_pilot.is_some() {
      rows.push(
        button(
          text(t!("industry.planner.unassign"))
            .font(typography::body::REGULAR)
            .size(typography::size::SM)
            .style(typography::colored(color::text::secondary())),
        )
        .width(Length::Fill)
        .padding(spacing::SPACE_2)
        .on_press(Message::OrderPilotAssigned {
          clone_id: None,
          index,
          pilot_id: None,
          type_id,
        })
        .style(pilot_row_style(false))
        .into(),
      );
    }

    if planner.pilots().is_empty() {
      rows.push(
        container(
          text(t!("industry.planner.pilots_empty"))
            .font(typography::body::REGULAR)
            .size(typography::size::SM)
            .style(typography::colored(color::text::tertiary())),
        )
        .padding(spacing::SPACE_2)
        .into(),
      );
    }

    for pilot in planner.pilots() {
      let is_expanded = expanded == Some(pilot.id);
      rows.push(pilot_header_row(type_id, index, pilot, is_expanded));
      if is_expanded {
        for clone in &pilot.clones {
          let selected = assigned_pilot == Some(pilot.id) && assigned_clone == clone.id;
          rows.push(clone_row(type_id, index, pilot.id, clone, selected));
        }
      }
    }

    container(
      scrollable(Column::with_children(rows).spacing(spacing::UNIT).width(Length::Fill))
        .style(crate::ui::style::control::scrollbar)
        .height(Length::Shrink),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_2)
    .max_height(PILOT_PICKER_MAX_HEIGHT)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      shadow: crate::ui::style::shadow::CARD,
      ..container::Style::default()
    })
    .into()
  }

  fn pilot_header_row<'a>(type_id: i64, index: usize, pilot: &'a PlanPilot, expanded: bool) -> Element<'a, Message> {
    button(
      Row::with_children(vec![
        avatar(pilot.id, &pilot.name, Length::Fixed(24.0), 24.0, pilot.portrait.clone()),
        Column::with_children(vec![
          text(pilot.name.clone())
            .font(typography::body::MEDIUM)
            .size(typography::size::MD)
            .style(typography::colored(color::text::PRIMARY))
            .into(),
          text(t!(
            "industry.planner.clone_count",
            count => pilot.clones.len(),
            plural => if pilot.clones.len() == 1 { "" } else { "s" }
          ))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
        ])
        .spacing(spacing::UNIT)
        .width(Length::Fill)
        .into(),
        Icon::chevron()
          .color(color::text::secondary())
          .size(13.0)
          .render::<Message>(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_2)
    .on_press(Message::OrderPilotPickerExpanded {
      pilot_id: pilot.id,
      index,
      type_id,
    })
    .style(pilot_row_style(expanded))
    .into()
  }

  fn clone_row<'a>(
    type_id: i64,
    index: usize,
    pilot_id: i64,
    clone: &'a PlanClone,
    selected: bool,
  ) -> Element<'a, Message> {
    let mut lines: Vec<Element<'a, Message>> = vec![
      text(clone.name.clone())
        .font(typography::body::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(if selected {
          color::accent::PLASMA
        } else {
          color::text::PRIMARY
        }))
        .into(),
      text(clone.implant_summary())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ];
    if let Some(location) = clone.location.as_ref().filter(|location| !location.is_empty()) {
      lines.push(
        text(location.clone())
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      );
    }

    let clone_id = clone.id;
    button(Column::with_children(lines).spacing(spacing::UNIT).width(Length::Fill))
      .width(Length::Fill)
      .padding(Padding {
        left: spacing::SPACE_6,
        right: spacing::SPACE_2,
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_2,
      })
      .on_press(Message::OrderPilotAssigned {
        clone_id,
        index,
        pilot_id: Some(pilot_id),
        type_id,
      })
      .style(pilot_row_style(selected))
      .into()
  }

  fn pilot_row_style(lit: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, status| {
      let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: (lit || hover).then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
        border: Border {
          radius: radius::CONTROL.into(),
          ..Border::default()
        },
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      }
    }
  }

  struct SegmentRow<'r> {
    count: usize,
    is_reaction: bool,
    recipe: &'r Recipe,
    te: i64,
    type_id: i64,
  }

  fn segment_row<'a>(
    planner: &'a Planner,
    ctx: &SegmentRow<'_>,
    index: usize,
    segment: &PlanSegment,
  ) -> Element<'a, Message> {
    let SegmentRow {
      count,
      is_reaction,
      recipe,
      te,
      type_id,
    } = *ctx;
    let rig_te_factor = planner
      .rig_factors_for(planner.settings_for(type_id).facility_structure)
      .te;
    let time = segment_build_time(
      recipe,
      segment.runs,
      te,
      rig_te_factor,
      planner.segment_assignment(segment),
    );
    let unit = if is_reaction {
      t!("industry.planner.unit_cycles")
    } else {
      t!("industry.planner.unit_runs")
    };

    let body = Row::with_children(vec![
      text("\u{2514}")
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::tertiary()))
        .into(),
      text(t!("industry.planner.split_index", index => index + 1, count => count))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
      segment_runs_field(type_id, index, segment.runs),
      text(unit)
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
      pilot_slot(planner, type_id, index, Some(segment)),
      Space::new().width(Length::Fill).into(),
      text(fmt_duration_coarse(time as i64))
        .font(typography::mono::REGULAR)
        .size(typography::size::SM)
        .style(typography::colored(color::text::secondary()))
        .into(),
      segment_remove_button(type_id, index),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .width(Length::Fill);

    container(body)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_2,
        left: SEGMENT_INDENT,
        right: spacing::SPACE_3,
      })
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          color: color::rule(),
          radius: 0.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn segment_runs_field<'a>(type_id: i64, index: usize, runs: i64) -> Element<'a, Message> {
    let value = runs.to_string();
    container(
      text_input("1", &value)
        .on_input(move |raw| Message::OrderSegmentRunsChanged {
          index,
          type_id,
          value: raw,
        })
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .align_x(Horizontal::Center)
        .padding(Padding {
          top: 0.0,
          bottom: 0.0,
          left: spacing::UNIT,
          right: spacing::UNIT,
        })
        .width(Length::Fixed(SEGMENT_RUNS_FIELD_WIDTH))
        .style(text_input_inner_style()),
    )
    .height(Length::Fixed(RUNS_STEPPER_HEIGHT))
    .align_y(Vertical::Center)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      border: Border {
        color: color::rule_strong(),
        radius: radius::CONTROL.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }

  fn segment_remove_button<'a>(type_id: i64, index: usize) -> Element<'a, Message> {
    button(
      container(
        text("\u{00D7}")
          .font(typography::mono::REGULAR)
          .size(typography::size::LG)
          .style(typography::colored(color::text::tertiary())),
      )
      .width(Length::Fixed(SEGMENT_REMOVE_BOX))
      .height(Length::Fixed(SEGMENT_REMOVE_BOX))
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center),
    )
    .padding(Padding::ZERO)
    .on_press(Message::OrderSegmentRemoved {
      index,
      type_id,
    })
    .style(|_, status| {
      let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: Some(Background::Color(iced::Color::TRANSPARENT)),
        border: Border {
          color: if hovered {
            color::with_alpha(color::status::DANGER, 0.42)
          } else {
            color::rule()
          },
          radius: radius::SUBTLE.into(),
          width: 1.0,
        },
        text_color: if hovered {
          color::status::DANGER
        } else {
          color::text::tertiary()
        },
        ..button::Style::default()
      }
    })
    .into()
  }

  fn runs_pill<'a>(runs: i64, is_reaction: bool, is_final: bool) -> Element<'a, Message> {
    let accent = if is_final {
      color::accent::PLASMA
    } else {
      color::text::PRIMARY
    };
    let label_color = if is_final {
      color::accent::PLASMA
    } else {
      color::text::secondary()
    };

    let inner = Column::with_children(vec![
      text(format!("\u{00D7}{}", fmt_num(runs)))
        .font(typography::mono::SEMIBOLD)
        .size(typography::size::LG)
        .style(typography::colored(accent))
        .into(),
      text(if is_reaction {
        t!("industry.planner.runs_label_cycles")
      } else {
        t!("industry.planner.runs_label_runs")
      })
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(label_color))
      .into(),
    ])
    .spacing(spacing::UNIT)
    .align_x(Horizontal::Center);

    let background = if is_final {
      color::with_alpha(color::accent::PLASMA, 0.18)
    } else {
      color::surface::SUNKEN
    };
    let border_color = if is_final {
      color::with_alpha(color::accent::PLASMA, 0.4)
    } else {
      color::rule()
    };

    container(inner)
      .padding(Padding {
        top: spacing::UNIT + 1.0,
        bottom: spacing::UNIT + 1.0,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      })
      .align_x(Horizontal::Center)
      .style(move |_| container::Style {
        background: Some(Background::Color(background)),
        border: Border {
          color: border_color,
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn merged_feeds_line(data: &PlannerData, job: &MergedBuildJob) -> String {
    if job.is_root {
      return t!("industry.planner.final_product").into_owned();
    }
    match job.consumers.as_slice() {
      [consumer] => t!("industry.planner.feeds_one", name => data.name(*consumer)).into_owned(),
      consumers => t!("industry.planner.feeds_jobs", count => consumers.len()).into_owned(),
    }
  }

  fn needed_blueprints<'a>(planner: &'a Planner) -> Element<'a, Message> {
    let data = planner.data();
    let blueprints = needed_blueprints_from(planner.merged_build_order());
    let count = blueprints.len();
    let missing = blueprints
      .iter()
      .filter(|bp| !data.owned.contains_key(&bp.type_id))
      .count();

    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for blueprint in &blueprints {
      rows.push(needed_blueprint_row(planner, blueprint));
    }

    let status = if missing > 0 {
      t!("industry.planner.to_acquire", count => missing).into_owned()
    } else {
      t!("industry.planner.all_owned").into_owned()
    };
    let hint = t!(
      "industry.planner.needed_blueprints_hint",
      count => count,
      plural => if count == 1 { "" } else { "s" },
      status => status
    )
    .into_owned();

    Column::with_children(vec![
      section_label(&t!("industry.planner.needed_blueprints"), Some(hint)),
      container(Column::with_children(rows).width(Length::Fill))
        .width(Length::Fill)
        .style(bordered_table)
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .into()
  }

  fn needed_blueprint_row<'a>(planner: &'a Planner, blueprint: &NeededBlueprint) -> Element<'a, Message> {
    let data = planner.data();
    let recipe = recipe_for(data, blueprint.type_id);
    let owned = data.owned.get(&blueprint.type_id);
    let kind_word = if recipe.is_reaction {
      t!("industry.planner.formula")
    } else {
      t!("industry.planner.blueprint")
    };
    let unit = if recipe.is_reaction {
      t!("industry.planner.unit_cycles")
    } else {
      t!("industry.planner.unit_runs")
    };

    let name_row = Row::with_children(vec![
      Row::with_children(vec![
        text(data.name(blueprint.type_id))
          .font(typography::body::MEDIUM)
          .size(typography::size::MD)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
        text(kind_word)
          .font(typography::body::REGULAR)
          .size(typography::size::MD)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      ])
      .spacing(spacing::UNIT + 1.0)
      .align_y(Vertical::Center)
      .into(),
      activity_badge(recipe.is_reaction),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center);

    let is_copy = owned.map(|summary| !summary.is_original).unwrap_or(false);
    let body = Row::with_children(vec![
      blueprint_tile(data.blueprint_icon(recipe.blueprint_type_id, is_copy)),
      Column::with_children(vec![
        name_row.into(),
        text(t!(
          "industry.planner.needed_blueprint_subtitle",
          jobs => blueprint.jobs,
          plural => if blueprint.jobs == 1 { "" } else { "s" },
          runs => fmt_num(blueprint.runs),
          unit => unit
        ))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
      ])
      .spacing(spacing::UNIT)
      .width(Length::Fill)
      .into(),
      blueprint_status_pill(owned),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

    let tinted = owned.is_none();
    container(body)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_3,
        bottom: spacing::SPACE_3,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      })
      .style(move |_| container::Style {
        background: tinted.then(|| Background::Color(color::with_alpha(color::status::WARNING, 0.06))),
        border: Border {
          color: color::rule(),
          radius: 0.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn blueprint_status_pill<'a>(owned: Option<&OwnedSummary>) -> Element<'a, Message> {
    match owned {
      Some(summary) => {
        let mut label = if summary.is_original {
          t!("industry.planner.status_bpo").into_owned()
        } else {
          t!("industry.planner.status_bpc").into_owned()
        };
        if summary.material_efficiency > 0 {
          label.push_str(&t!("industry.planner.status_me", me => summary.material_efficiency));
        }
        if !summary.in_scope {
          label.push_str(&t!("industry.planner.elsewhere"));
        }
        badge(
          label,
          Some(if summary.in_scope {
            color::status::ONLINE
          } else {
            color::text::secondary()
          }),
        )
      }
      None => badge(t!("industry.planner.buy_invent"), Some(color::status::WARNING)),
    }
  }

  fn blueprint_tile<'a>(resolution: &IconResolution) -> Element<'a, Message> {
    match resolution {
      IconResolution::Found(path) => icon_tile(
        clip_layer(
          image(image::Handle::from_path(path.clone()))
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Cover),
          Length::Fill,
          Length::Fill,
        ),
        TILE_BOX,
      ),
      IconResolution::Missing => icon_tile(
        Icon::copy()
          .color(color::text::tertiary())
          .size(TILE_BOX * 0.45)
          .render::<Message>(),
        TILE_BOX,
      ),
    }
  }

  fn right_pane<'a>(planner: &'a Planner, product: Option<i64>) -> Element<'a, Message> {
    let active = planner.right_tab();
    let plans_count = match planner.saved().len() {
      0 => String::new(),
      count => count.to_string(),
    };
    let tabs = container(tab_select_with(
      vec![
        right_tab("Detail", active == RightTab::Detail, RightTab::Detail, String::new()),
        right_tab("Plans", active == RightTab::Plans, RightTab::Plans, plans_count),
      ],
      TabLayout::Fill,
    ))
    .width(Length::Fill)
    .height(Length::Fixed(TAB_STRIP_HEIGHT));

    let content: Element<'a, Message> = match planner.right_tab() {
      RightTab::Detail => match product {
        Some(product) => detail_pane(planner, product),
        None => centered(
          text(t!("industry.planner.detail_empty"))
            .font(typography::body::REGULAR)
            .size(typography::size::MD)
            .style(typography::colored(color::text::tertiary())),
        ),
      },
      RightTab::Plans => plans_pane(planner),
    };

    let column = Column::with_children(vec![
      tabs.into(),
      rule::horizontal(),
      container(scrollable(content).style(crate::ui::style::control::scrollbar))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(spacing::SPACE_6)
        .into(),
    ])
    .width(Length::Fill)
    .height(Length::Fill);

    container(column)
      .width(Length::Fixed(planner.detail_pane_width()))
      .height(Length::Fill)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::SUNKEN)),
        border: Border {
          color: color::rule(),
          radius: 0.0.into(),
          width: 1.0,
        },
        ..container::Style::default()
      })
      .into()
  }

  fn detail_pane<'a>(planner: &'a Planner, product: i64) -> Element<'a, Message> {
    let Some(eco) = planner.economics() else {
      return Space::new().into();
    };
    let data = planner.data();

    let header = Row::with_children(vec![
      type_tile(data.type_icon(product)),
      Column::with_children(vec![
        text(data.name(product))
          .font(typography::body::MEDIUM)
          .size(typography::size::LG)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
        text(t!(
          "industry.planner.detail_subtitle",
          output => fmt_num(eco.output_qty),
          runs => planner.runs()
        ))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
      ])
      .spacing(spacing::UNIT)
      .into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

    let profit_color = if eco.profitable() {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };
    let hero = container(
      Column::with_children(vec![
        micro_label(&t!("industry.planner.estimated_profit")),
        text(format!(
          "{}{}",
          if eco.profit >= 0.0 { "+" } else { "\u{2212}" },
          fmt_isk(eco.profit.abs())
        ))
        .font(typography::mono::SEMIBOLD)
        .size(typography::size::LG)
        .style(typography::colored(profit_color))
        .into(),
        Row::with_children(vec![
          text(t!("industry.planner.margin", pct => fmt_pct(eco.margin)))
            .font(typography::mono::MEDIUM)
            .size(typography::size::MD)
            .style(typography::colored(profit_color))
            .into(),
          text(t!("industry.planner.per_unit", value => fmt_isk(eco.per_unit)))
            .font(typography::mono::REGULAR)
            .size(typography::size::MD)
            .style(typography::colored(color::text::secondary()))
            .into(),
        ])
        .spacing(spacing::SPACE_3)
        .into(),
      ])
      .spacing(spacing::SPACE_2),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_3_5)
    .style(move |_| container::Style {
      background: Some(Background::Color(color::with_alpha(profit_color, 0.08))),
      border: Border {
        color: color::with_alpha(profit_color, 0.3),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    });

    let breakdown = Column::with_children(vec![
      detail_line(
        &t!("industry.planner.revenue"),
        &fmt_isk_full(eco.revenue),
        color::text::PRIMARY,
        false,
      ),
      detail_line(
        &t!("industry.planner.material_cost"),
        &format!("\u{2212}{}", fmt_isk_full(eco.material_cost)),
        color::status::DANGER,
        false,
      ),
      detail_line(
        &t!("industry.planner.job_fee"),
        &format!("\u{2212}{}", fmt_isk_full(eco.install_fee)),
        color::status::DANGER,
        false,
      ),
      detail_line(
        &t!("industry.planner.net_profit"),
        &format!(
          "{}{}",
          if eco.profit >= 0.0 { "+" } else { "\u{2212}" },
          fmt_isk_full(eco.profit.abs())
        ),
        profit_color,
        true,
      ),
    ])
    .spacing(spacing::SPACE_2);

    let meta = Column::with_children(vec![
      meta_line(
        Icon::clock(),
        &t!("industry.planner.meta_build_time"),
        &fmt_duration_coarse(eco.build_time_secs as i64),
      ),
      meta_line(
        Icon::wallet(),
        &t!("industry.planner.isk_per_hour"),
        &fmt_isk(eco.isk_per_hour()),
      ),
      meta_line(
        Icon::assets(),
        &t!("industry.planner.meta_output_volume"),
        &fmt_volume(eco.output_volume),
      ),
    ])
    .spacing(spacing::SPACE_2);

    Column::with_children(vec![
      header.into(),
      ownership_strip(planner, product),
      hero.into(),
      breakdown.into(),
      rule::horizontal(),
      meta.into(),
      copy_button(),
    ])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
  }

  fn ownership_strip<'a>(planner: &'a Planner, product: i64) -> Element<'a, Message> {
    let owned = planner.data().owned.get(&product);
    let (icon, accent, title, sub) = match owned {
      Some(summary) => (
        Icon::check(),
        if summary.in_scope {
          color::status::ONLINE
        } else {
          color::text::secondary()
        },
        if summary.is_original {
          t!("industry.planner.blueprint_original").into_owned()
        } else {
          t!("industry.planner.blueprint_copy").into_owned()
        },
        t!(
          "industry.planner.ownership_sub",
          kind => if summary.is_original {
            t!("industry.planner.status_bpo")
          } else {
            t!("industry.planner.status_bpc")
          },
          scope => if summary.in_scope {
            t!("industry.planner.in_scope")
          } else {
            t!("industry.planner.held_elsewhere")
          },
          me => summary.material_efficiency,
          te => summary.time_efficiency
        )
        .into_owned(),
      ),
      None => (
        Icon::help(),
        color::status::WARNING,
        t!("industry.planner.blueprint_no_owned").into_owned(),
        t!("industry.planner.blueprint_no_owned_sub").into_owned(),
      ),
    };

    let ok = owned.is_some();
    container(
      Row::with_children(vec![
        icon.color(accent).size(14.0).render::<Message>(),
        Column::with_children(vec![
          text(title)
            .font(typography::body::REGULAR)
            .size(typography::size::MD)
            .style(typography::colored(color::text::PRIMARY))
            .into(),
          text(sub)
            .font(typography::mono::REGULAR)
            .size(typography::size::XS)
            .style(typography::colored(color::text::tertiary()))
            .into(),
        ])
        .spacing(spacing::UNIT)
        .width(Length::Fill)
        .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_2_5)
    .style(move |_| container::Style {
      background: (!ok).then(|| Background::Color(color::with_alpha(color::status::WARNING, 0.08))),
      border: Border {
        color: if ok {
          color::rule()
        } else {
          color::with_alpha(color::status::WARNING, 0.34)
        },
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }

  fn plans_pane(planner: &Planner) -> Element<'_, Message> {
    let saved = planner.saved();
    let body: Element<'_, Message> = if saved.is_empty() {
      plans_empty()
    } else {
      let rows: Vec<Element<'_, Message>> = saved.iter().map(|plan| plan_row(planner, plan)).collect();
      Column::with_children(rows).spacing(spacing::SPACE_3).into()
    };

    Column::with_children(vec![save_plan_button(planner), body])
      .spacing(spacing::SPACE_3_5)
      .width(Length::Fill)
      .into()
  }

  fn save_plan_button(planner: &Planner) -> Element<'_, Message> {
    Button::primary(t!("industry.planner.save_build_plan").into_owned())
      .icon(Icon::plus())
      .size(Size::Md)
      .block()
      .on_press_maybe(planner.product().is_some().then_some(Message::PlanSaveRequested))
      .into()
  }

  fn plan_row<'a>(planner: &'a Planner, plan: &'a SavedPlan) -> Element<'a, Message> {
    let header = Row::with_children(vec![
      type_tile(planner.data().type_icon(plan.product_type_id)),
      Column::with_children(vec![
        text(plan.name.clone())
          .font(typography::body::MEDIUM)
          .size(typography::size::MD)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
        text(planner.data().name(plan.product_type_id))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      ])
      .spacing(spacing::UNIT)
      .width(Length::Fill)
      .into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center);

    let economics = plan_economics(plan.economics.as_ref());

    let actions = Row::with_children(vec![
      plan_action(
        &t!("industry.planner.plan_action_load"),
        color::accent::PLASMA,
        Message::PlanLoadRequested(plan.id),
      ),
      plan_action(
        &t!("industry.planner.plan_action_delete"),
        color::status::DANGER,
        Message::PlanDeleteRequested(plan.id),
      ),
    ])
    .spacing(spacing::SPACE_2);

    container(
      Column::with_children(vec![header.into(), economics, actions.into()])
        .spacing(spacing::SPACE_3)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_3_5)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::RAISED)),
      border: Border {
        color: color::rule(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }

  fn plan_economics(economics: Option<&Economics>) -> Element<'_, Message> {
    let Some(eco) = economics else {
      return text(t!("industry.planner.plan_economics_unavailable"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(typography::colored(color::text::tertiary()))
        .into();
    };
    let profit_color = if eco.profitable() {
      color::status::ONLINE
    } else {
      color::status::DANGER
    };

    Row::with_children(vec![
      metric(
        &t!("industry.planner.metric_profit"),
        &format!(
          "{}{}",
          if eco.profit >= 0.0 { "+" } else { "\u{2212}" },
          fmt_isk(eco.profit.abs())
        ),
        profit_color,
      ),
      metric(
        &t!("industry.planner.metric_margin"),
        &fmt_pct(eco.margin),
        profit_color,
      ),
      metric(
        &t!("industry.planner.metric_revenue"),
        &fmt_isk(eco.revenue),
        color::text::secondary(),
      ),
    ])
    .spacing(spacing::SPACE_6)
    .into()
  }

  fn plan_action<'a>(label: &str, accent: iced::Color, message: Message) -> Element<'a, Message> {
    button(
      text(label.to_owned())
        .font(typography::body::MEDIUM)
        .size(typography::size::SM)
        .style(typography::colored(accent)),
    )
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .on_press(message)
    .style(move |_, status| {
      let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: hovered.then(|| Background::Color(color::with_alpha(accent, 0.14))),
        border: Border {
          color: color::with_alpha(accent, 0.45),
          radius: radius::CONTROL.into(),
          width: 1.0,
        },
        text_color: accent,
        ..button::Style::default()
      }
    })
    .into()
  }

  fn plans_empty<'a>() -> Element<'a, Message> {
    centered(
      Column::with_children(vec![
        Icon::doc()
          .color(color::text::tertiary())
          .size(28.0)
          .render::<Message>(),
        text(t!("industry.planner.plans_empty"))
          .font(typography::body::REGULAR)
          .size(typography::size::LG)
          .style(typography::colored(color::text::secondary()))
          .into(),
        text(t!("industry.planner.plans_empty_sub"))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      ])
      .spacing(spacing::SPACE_3)
      .align_x(Horizontal::Center),
    )
  }

  fn copy_button<'a>() -> Element<'a, Message> {
    Button::secondary(t!("industry.planner.copy_shopping_list").into_owned())
      .icon(Icon::copy())
      .size(Size::Md)
      .block()
      .on_press(Message::ShoppingListCopied)
      .into()
  }

  fn detail_line<'a>(label: &str, value: &str, value_color: iced::Color, emphasized: bool) -> Element<'a, Message> {
    let label_color = if emphasized {
      color::text::PRIMARY
    } else {
      color::text::secondary()
    };
    container(
      Row::with_children(vec![
        text(label.to_owned())
          .font(if emphasized {
            typography::body::MEDIUM
          } else {
            typography::body::REGULAR
          })
          .size(typography::size::MD)
          .style(typography::colored(label_color))
          .into(),
        Space::new().width(Length::Fill).into(),
        text(value.to_owned())
          .font(typography::mono::MEDIUM)
          .size(typography::size::MD)
          .style(typography::colored(value_color))
          .into(),
      ])
      .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: 0.0,
      right: 0.0,
    })
    .style(|_| container::Style {
      border: Border {
        color: color::rule(),
        radius: 0.0.into(),
        width: 1.0,
      },
      ..container::Style::default()
    })
    .into()
  }

  fn meta_line<'a>(icon: Icon, label: &str, value: &str) -> Element<'a, Message> {
    Row::with_children(vec![
      icon.color(color::text::secondary()).size(15.0).render::<Message>(),
      text(label.to_owned())
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::secondary()))
        .into(),
      Space::new().width(Length::Fill).into(),
      text(value.to_owned())
        .font(typography::mono::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    ])
    .spacing(spacing::SPACE_2)
    .align_y(Vertical::Center)
    .into()
  }

  fn right_tab<'a>(label: &'a str, selected: bool, target: RightTab, count: String) -> Tab<'a, Message> {
    Tab {
      count,
      icon: None,
      label,
      on_press: Some(Message::RightTabSelected(target)),
      selected,
    }
  }

  fn metric<'a>(label: &str, value: &str, value_color: iced::Color) -> Element<'a, Message> {
    Column::with_children(vec![
      micro_label(label),
      text(value.to_owned())
        .font(typography::mono::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(value_color))
        .into(),
    ])
    .spacing(spacing::UNIT)
    .into()
  }

  fn grid_header<'a>() -> Element<'a, Message> {
    let head = |label: &str, width: Option<f32>, right: bool| -> Element<'a, Message> {
      let content = text(label.to_owned())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()));
      match width {
        Some(width) => container(container(content).width(Length::Fill).align_x(if right {
          Horizontal::Right
        } else {
          Horizontal::Left
        }))
        .width(Length::Fixed(width))
        .into(),
        None => container(content).width(Length::Fill).into(),
      }
    };

    container(
      Row::with_children(vec![
        head(&t!("industry.planner.grid_material"), None, false),
        head(&t!("industry.planner.grid_quantity"), Some(COL_QTY), true),
        head(&t!("industry.planner.grid_unit_price"), Some(COL_PRICE), true),
        head(&t!("industry.planner.grid_subtotal"), Some(COL_COST), true),
      ])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_2,
      bottom: spacing::SPACE_2,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
  }

  fn grid_value<'a>(value: &str, width: f32, value_color: iced::Color) -> Element<'a, Message> {
    container(
      container(
        text(value.to_owned())
          .font(typography::mono::REGULAR)
          .size(typography::size::MD)
          .style(typography::colored(value_color)),
      )
      .width(Length::Fill)
      .align_x(Horizontal::Right),
    )
    .width(Length::Fixed(width))
    .into()
  }

  fn footer_row<'a>(label: &str, value: &str) -> Element<'a, Message> {
    container(
      Row::with_children(vec![
        text(label.to_uppercase())
          .font(typography::mono::REGULAR)
          .size(typography::size::SM)
          .style(typography::colored(color::text::secondary()))
          .into(),
        Space::new().width(Length::Fill).into(),
        text(value.to_owned())
          .font(typography::mono::MEDIUM)
          .size(typography::size::LG)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
      ])
      .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_3,
      right: spacing::SPACE_3,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::SUNKEN)),
      ..container::Style::default()
    })
    .into()
  }

  fn section_label<'a>(label: &str, hint: Option<String>) -> Element<'a, Message> {
    let mut children: Vec<Element<'a, Message>> = vec![
      text(label.to_uppercase())
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
    ];
    if let Some(hint) = hint {
      children.push(
        text(t!("industry.planner.section_hint", hint => hint))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary()))
          .into(),
      );
    }
    Row::with_children(children)
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center)
      .into()
  }

  fn micro_label<'a>(label: &str) -> Element<'a, Message> {
    text(label.to_uppercase())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::secondary()))
      .into()
  }

  fn activity_badge<'a>(is_reaction: bool) -> Element<'a, Message> {
    if is_reaction {
      badge(t!("industry.planner.reaction"), Some(color::status::WARNING))
    } else {
      badge(t!("industry.planner.manufacturing"), Some(color::accent::PLASMA))
    }
  }

  fn owned_badge<'a>(planner: &Planner, type_id: i64) -> Element<'a, Message> {
    match planner.data().owned.get(&type_id) {
      Some(summary) => badge(
        if summary.is_original {
          t!("industry.planner.status_bpo")
        } else {
          t!("industry.planner.status_bpc")
        },
        Some(if summary.in_scope {
          color::status::ONLINE
        } else {
          color::text::secondary()
        }),
      ),
      None => badge(t!("industry.planner.no_bp"), None),
    }
  }

  fn type_tile<'a>(resolution: &IconResolution) -> Element<'a, Message> {
    match resolution {
      IconResolution::Found(path) => icon_tile(
        clip_layer(
          image(image::Handle::from_path(path.clone()))
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Cover),
          Length::Fill,
          Length::Fill,
        ),
        TILE_BOX,
      ),
      IconResolution::Missing => icon_tile(
        Icon::inventory()
          .color(color::text::tertiary())
          .size(TILE_BOX * 0.5)
          .render::<Message>(),
        TILE_BOX,
      ),
    }
  }

  fn recipe_for(data: &PlannerData, type_id: i64) -> Recipe {
    data.recipe(type_id).cloned().unwrap_or(Recipe {
      activity_id: 1,
      blueprint_type_id: 0,
      is_reaction: false,
      materials: Vec::new(),
      output_per_run: 1,
      time_per_run: 0,
    })
  }

  fn bordered_table(_: &iced::Theme) -> container::Style {
    container::Style {
      border: Border {
        color: color::rule(),
        radius: radius::CARD.into(),
        width: 1.0,
      },
      ..container::Style::default()
    }
  }

  fn picker_row_style(_: &iced::Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
      background: hovered.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.1))),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      text_color: color::text::PRIMARY,
      ..button::Style::default()
    }
  }

  fn empty_left(planner: &Planner) -> Element<'_, Message> {
    let mut children: Vec<Element<'_, Message>> = vec![picker(planner)];

    if !planner.picker_open() && planner.search().is_empty() {
      children.push(centered(
        text(t!("industry.planner.empty"))
          .font(typography::body::REGULAR)
          .size(typography::size::LG)
          .style(typography::colored(color::text::tertiary())),
      ));
    }

    Column::with_children(children)
      .spacing(spacing::SPACE_3)
      .padding(PANE_PADDING)
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  }

  fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content.into())
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(Horizontal::Center)
      .align_y(Vertical::Center)
      .padding(spacing::SPACE_6)
      .into()
  }

  fn matches_category(category: Category, active: Category) -> bool {
    active == Category::Other || category == active
  }

  pub(super) fn fmt_num(value: i64) -> String {
    let mut out = String::new();
    let digits = value.abs().to_string();
    let bytes = digits.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
      if index > 0 && (bytes.len() - index).is_multiple_of(3) {
        out.push(',');
      }
      out.push(*byte as char);
    }
    if value < 0 { format!("-{out}") } else { out }
  }

  fn fmt_price(value: f64) -> String {
    if value < 100.0 {
      format!("{value:.2}")
    } else {
      fmt_num(value.round() as i64)
    }
  }

  fn fmt_pct(value: f64) -> String {
    format!("{value:.1}%")
  }

  #[cfg(test)]
  mod tests {
    use iced::advanced::widget::Tree;

    use super::*;
    use crate::features::industry::planner_model::BuildNode;

    #[test]
    fn it_headers_your_blueprints_when_empty_and_unsearched() {
      assert_eq!(picker_header("", 0), "Your blueprints / recent");
    }

    #[test]
    fn it_headers_a_singular_result_count() {
      assert_eq!(picker_header("trit", 1), "1 result");
    }

    #[test]
    fn it_headers_a_plural_result_count() {
      assert_eq!(picker_header("trit", 3), "3 results");
    }

    #[test]
    fn it_labels_a_missing_blueprint_for_acquisition() {
      let none = blueprint_status_pill(None);
      let _ = Tree::new(none.as_widget());

      let owned = blueprint_status_pill(Some(&OwnedSummary {
        in_scope: false,
        is_original: true,
        material_efficiency: 10,
        time_efficiency: 20,
      }));
      let _ = Tree::new(owned.as_widget());
    }

    #[test]
    fn it_renders_the_multi_consumer_feeds_line_as_a_job_count() {
      let data = PlannerData::default();
      let job = MergedBuildJob {
        consumers: vec![42, 43],
        is_root: false,
        needed_qty: 10,
        node: BuildNode::new(7, 1, false, Vec::new()),
        runs: 1,
        type_id: 7,
      };

      assert_eq!(merged_feeds_line(&data, &job), "feeds \u{2192} 2 jobs");
    }

    #[test]
    fn it_renders_the_root_feeds_line_as_the_final_product() {
      let data = PlannerData::default();
      let job = MergedBuildJob {
        consumers: Vec::new(),
        is_root: true,
        needed_qty: 1,
        node: BuildNode::new(7, 1, false, Vec::new()),
        runs: 1,
        type_id: 7,
      };

      assert_eq!(merged_feeds_line(&data, &job), "final product");
    }

    #[test]
    fn it_renders_the_single_consumer_feeds_line() {
      let mut data = PlannerData::default();
      data.names.insert(42, "Hulk".to_owned());
      let job = MergedBuildJob {
        consumers: vec![42],
        is_root: false,
        needed_qty: 10,
        node: BuildNode::new(7, 1, false, Vec::new()),
        runs: 1,
        type_id: 7,
      };

      assert_eq!(merged_feeds_line(&data, &job), "feeds \u{2192} Hulk");
    }

    fn plan_clone(id: Option<i64>, name: &str, implants: &[&str]) -> PlanClone {
      PlanClone {
        id,
        implant_names: implants.iter().map(|implant| (*implant).to_owned()).collect(),
        location: Some("Jita IV - Moon 4".to_owned()),
        name: name.to_owned(),
        ..PlanClone::default()
      }
    }

    fn plan_pilot(id: i64, name: &str) -> PlanPilot {
      PlanPilot {
        clones: vec![
          plan_clone(None, "Active clone", &["Zainou 'Beancounter'"]),
          plan_clone(Some(7), "Industry clone", &[]),
        ],
        id,
        name: name.to_owned(),
        portrait: None,
        ..PlanPilot::default()
      }
    }

    fn assignable_planner() -> Planner {
      let mut planner = Planner::new();
      planner.set_assign_pilots(true);
      planner.set_pilots(vec![plan_pilot(1, "Miner Joe"), plan_pilot(2, "Hauler Sue")]);
      planner
    }

    #[test]
    fn it_renders_the_unassigned_pilot_trigger() {
      let trigger = pilot_trigger(22_544, 0, None);
      let _ = Tree::new(trigger.as_widget());
    }

    #[test]
    fn it_renders_an_assigned_pilot_trigger_with_a_named_clone() {
      let pilot = plan_pilot(1, "Miner Joe");
      let clone = &pilot.clones[1];
      let trigger = pilot_trigger(22_544, 0, Some((&pilot, Some(clone))));
      let _ = Tree::new(trigger.as_widget());
    }

    #[test]
    fn it_renders_an_assigned_pilot_trigger_with_the_active_clone() {
      let pilot = plan_pilot(1, "Miner Joe");
      let trigger = pilot_trigger(22_544, 0, Some((&pilot, None)));
      let _ = Tree::new(trigger.as_widget());
    }

    #[test]
    fn it_renders_the_popover_with_unassigned_segment_and_pilots() {
      let planner = assignable_planner();
      let segment = PlanSegment::unassigned(10);
      let popover = pilot_popover(&planner, 22_544, 0, Some(&segment));
      let _ = Tree::new(popover.as_widget());
    }

    #[test]
    fn it_renders_the_popover_with_an_assigned_pilot_and_unassign_action() {
      let planner = assignable_planner();
      let segment = PlanSegment {
        clone_id: Some(7),
        pilot_id: Some(1),
        runs: 10,
      };
      let popover = pilot_popover(&planner, 22_544, 0, Some(&segment));
      let _ = Tree::new(popover.as_widget());
    }

    #[test]
    fn it_renders_the_popover_empty_state_without_pilots_in_scope() {
      let planner = Planner::new();
      let popover = pilot_popover(&planner, 22_544, 0, None);
      let _ = Tree::new(popover.as_widget());
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::industry::{
    planner_loaders::{CatalogEntry, OwnedSummary},
    planner_model::Material,
  };

  const COMPONENT: i64 = 11_000;

  const HULK: i64 = 22_544;

  const RETRIEVER: i64 = 17_478;

  const TRITANIUM: i64 = 34;

  fn facility(id: i64, solar_system_id: i64, name: &str, manufacturing_index: f64) -> PlannerFacility {
    PlannerFacility {
      id,
      manufacturing_index: Some(manufacturing_index),
      name: name.to_owned(),
      reaction_index: Some(manufacturing_index),
      region: None,
      security_status: None,
      solar_system: None,
      solar_system_id,
      type_id: None,
      type_label: None,
    }
  }

  fn recipe(blueprint: i64, output: i64, is_reaction: bool, materials: Vec<Material>) -> Recipe {
    Recipe {
      activity_id: if is_reaction { 11 } else { 1 },
      blueprint_type_id: blueprint,
      is_reaction,
      materials,
      output_per_run: output,
      time_per_run: 100,
    }
  }

  fn planner() -> Planner {
    let mut data = PlannerData::default();
    data.recipes.insert(
      HULK,
      recipe(
        HULK + 1,
        1,
        false,
        vec![Material::new(RETRIEVER, 2), Material::new(TRITANIUM, 5)],
      ),
    );
    data.recipes.insert(
      RETRIEVER,
      recipe(RETRIEVER + 1, 1, false, vec![Material::new(TRITANIUM, 10)]),
    );
    data.prices.insert(HULK, 200_000_000.0);
    data.prices.insert(RETRIEVER, 30_000_000.0);
    data.prices.insert(TRITANIUM, 5.0);
    data.names.insert(HULK, "Hulk".to_owned());
    data.names.insert(RETRIEVER, "Retriever".to_owned());
    data.names.insert(TRITANIUM, "Tritanium".to_owned());
    data.catalog.push(CatalogEntry {
      category: Category::Ship,
      group_name: "Mining Barge".to_owned(),
      is_reaction: false,
      name: "Hulk".to_owned(),
      type_id: HULK,
      volume: 3_750.0,
    });
    data.facilities = vec![
      facility(60_000_002, 30_002_187, "Cheap Citadel", 0.02),
      facility(60_000_001, 30_000_142, "Pricey Station", 0.09),
    ];

    let mut planner = Planner::new();
    planner.apply_data(data);
    planner.update(Message::ProductPicked(HULK));
    planner.update(Message::RunsChanged(1));
    planner
  }

  mod rig_application {
    use pretty_assertions::assert_eq;

    use super::*;

    fn rigged_planner(security_status: Option<f64>) -> Planner {
      let mut data = PlannerData::default();
      data
        .recipes
        .insert(HULK, recipe(HULK + 1, 1, false, vec![Material::new(TRITANIUM, 5)]));
      data.prices.insert(HULK, 200_000_000.0);
      data.prices.insert(TRITANIUM, 10.0);
      data.adjusted_prices.insert(TRITANIUM, 10.0);
      data.names.insert(HULK, "Hulk".to_owned());
      data.names.insert(TRITANIUM, "Tritanium".to_owned());
      let mut hub = facility(60_000_002, 30_002_187, "Rigged Hub", 0.05);
      hub.security_status = security_status;
      data.facilities = vec![hub];

      let mut planner = Planner::new();
      planner.set_facility_defaults(FacilityDefaults {
        manufacturing: Some(60_000_002),
        reactions: None,
      });
      planner.apply_data(data);
      planner.update(Message::ProductPicked(HULK));
      planner
    }

    fn catalog() -> HashMap<i64, RigBonus> {
      HashMap::from([(
        9001,
        RigBonus {
          fee: -10.0,
          me: -2.0,
          te: -20.0,
        },
      )])
    }

    #[test]
    fn it_reduces_the_install_fee_for_a_rigged_structure() {
      let mut planner = rigged_planner(Some(0.9));
      let baseline = planner.economics().expect("baseline economics");

      planner.set_rig_data(HashMap::from([(60_000_002, vec![9001])]), catalog());
      let rigged = planner.economics().expect("rigged economics");

      assert!(rigged.install_fee < baseline.install_fee);
    }

    #[test]
    fn it_leaves_an_untracked_structure_install_fee_unchanged() {
      let mut planner = rigged_planner(Some(0.9));
      let baseline = planner.economics().expect("baseline economics");

      planner.set_rig_data(HashMap::from([(70_000_000, vec![9001])]), catalog());
      let untracked = planner.economics().expect("untracked economics");

      assert_eq!(untracked.install_fee, baseline.install_fee);
    }

    #[test]
    fn it_leaves_an_unrigged_structure_install_fee_unchanged() {
      let mut planner = rigged_planner(Some(0.9));
      let baseline = planner.economics().expect("baseline economics");

      planner.set_rig_data(HashMap::from([(60_000_002, Vec::new())]), catalog());
      let unrigged = planner.economics().expect("unrigged economics");

      assert_eq!(unrigged.install_fee, baseline.install_fee);
    }

    #[test]
    fn it_scales_the_rig_bonus_by_the_low_sec_band() {
      let mut planner = rigged_planner(Some(0.4));

      planner.set_rig_data(HashMap::from([(60_000_002, vec![9001])]), catalog());
      let factors = planner.rig_factors_for(Some(60_000_002));

      assert!((factors.me - (1.0 + -2.0 * 1.9 / 100.0)).abs() < 1e-9);
      assert!((factors.te - (1.0 + -20.0 * 1.9 / 100.0)).abs() < 1e-9);
      assert!((factors.fee - (1.0 + -10.0 * 1.9 / 100.0)).abs() < 1e-9);
    }
  }

  mod apply_data {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lands_on_the_empty_state_without_auto_selecting_a_product() {
      let mut planner = Planner::new();

      planner.apply_data(PlannerData::default());

      assert_eq!(planner.product(), None);
    }

    #[test]
    fn it_still_seeds_the_picker_recent_list() {
      let mut data = PlannerData::default();
      data.catalog.push(CatalogEntry {
        category: Category::Ship,
        group_name: "Mining Barge".to_owned(),
        is_reaction: false,
        name: "Hulk".to_owned(),
        type_id: HULK,
        volume: 3_750.0,
      });
      let mut planner = Planner::new();

      planner.apply_data(data);

      assert_eq!(planner.recent(), &[HULK]);
      assert_eq!(planner.product(), None);
    }
  }

  mod break_down {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ignores_a_breakdown_request_for_a_raw_material() {
      let mut planner = planner();

      planner.update(Message::NodeBrokenDown {
        type_id: TRITANIUM,
      });

      assert!(!planner.is_built(TRITANIUM));
    }

    #[test]
    fn it_nests_a_buildable_child_with_runs_locked_to_parent_demand() {
      let mut planner = planner();
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        type_id: HULK,
      });

      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      let plan = planner.plan().unwrap();
      let builds = plan.collect_builds();
      assert_eq!(builds.len(), 1);
      assert_eq!(builds[0].type_id, RETRIEVER);
      assert_eq!(builds[0].needed_qty, 2);
      assert_eq!(builds[0].runs, 2);
    }

    #[test]
    fn it_rolls_a_broken_down_child_into_raw_totals() {
      let mut planner = planner();
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        type_id: HULK,
      });
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        type_id: RETRIEVER,
      });

      let totals = planner.plan().unwrap().raw_totals();
      let tritanium = totals.iter().find(|t| t.type_id == TRITANIUM).unwrap();
      assert_eq!(tritanium.qty, 25);
      assert!(totals.iter().all(|t| t.type_id != RETRIEVER));
    }
  }

  mod break_down_all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_breaks_down_a_reaction_input() {
      const FUEL: i64 = 4051;
      const COMPOSITE: i64 = 16_670;
      const GAS: i64 = 25_268;
      let mut data = PlannerData::default();
      data
        .recipes
        .insert(FUEL, recipe(FUEL + 1, 40, true, vec![Material::new(COMPOSITE, 25)]));
      data.recipes.insert(
        COMPOSITE,
        recipe(COMPOSITE + 1, 100, true, vec![Material::new(GAS, 50)]),
      );
      data.names.insert(FUEL, "Fuel Block".to_owned());
      data.names.insert(COMPOSITE, "Composite".to_owned());
      data.names.insert(GAS, "Gas".to_owned());
      let mut planner = Planner::new();
      planner.apply_data(data);
      planner.update(Message::ProductPicked(FUEL));

      planner.update(Message::BreakDownAll);

      assert!(planner.is_built(COMPOSITE));
      let root = planner.plan().unwrap().root;
      assert!(root.children.contains_key(&COMPOSITE));
      assert!(root.children[&COMPOSITE].children.is_empty());
    }

    #[test]
    fn it_is_idempotent_and_keeps_existing_breakdowns() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      let before = planner.plan();

      planner.update(Message::BreakDownAll);

      assert_eq!(planner.plan(), before);
    }

    #[test]
    fn it_recursively_breaks_down_every_buildable_input_to_raw() {
      let mut planner = planner();

      planner.update(Message::BreakDownAll);

      assert!(planner.is_built(RETRIEVER));
      assert!(!planner.is_built(TRITANIUM));
      let root = planner.plan().unwrap().root;
      assert!(root.children.contains_key(&RETRIEVER));
      assert!(root.children[&RETRIEVER].children.is_empty());
    }

    #[test]
    fn it_reports_buildable_inputs_only_when_present() {
      let planner = planner();
      assert!(planner.has_buildable_inputs());

      let mut data = PlannerData::default();
      data.recipes.insert(
        TRITANIUM,
        recipe(TRITANIUM + 1, 1, false, vec![Material::new(99_999, 1)]),
      );
      data.names.insert(TRITANIUM, "Tritanium".to_owned());
      let mut raw_only = Planner::new();
      raw_only.apply_data(data);
      raw_only.update(Message::ProductPicked(TRITANIUM));
      assert!(!raw_only.has_buildable_inputs());
    }
  }

  mod collapse {
    use super::*;

    #[test]
    fn it_restores_a_collapsed_child_to_a_raw_input() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      planner.update(Message::NodeCollapsed {
        type_id: RETRIEVER,
      });

      let totals = planner.plan().unwrap().raw_totals();
      assert!(totals.iter().any(|t| t.type_id == RETRIEVER));
    }
  }

  mod detail_pane {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::shell::window_state::UiState;

    const HOST: f32 = 1_200.0;

    #[test]
    fn it_clamps_a_stored_width_below_the_minimum() {
      let mut ui = UiState::default();
      ui.panes.insert("main".to_owned(), HOST);
      ui.panes.insert(DETAIL_PANE_KEY.to_owned(), 0.01);

      let mut planner = Planner::new().with_restored_panes(&ui);
      planner.set_pane_host_width(HOST);

      assert_eq!(planner.detail_pane_width(), DETAIL_PANE_MIN_WIDTH);
    }

    #[test]
    fn it_restores_the_detail_pane_width_from_the_keyed_store() {
      let mut ui = UiState::default();
      ui.panes.insert("main".to_owned(), HOST);
      ui.panes.insert(DETAIL_PANE_KEY.to_owned(), 0.3);

      let mut planner = Planner::new().with_restored_panes(&ui);
      planner.set_pane_host_width(HOST);

      assert_eq!(planner.detail_pane_width(), 360.0);
    }

    #[test]
    fn it_settles_and_round_trips_a_dragged_width_through_the_store() {
      let mut ui = UiState::default();
      ui.panes.insert("main".to_owned(), HOST);
      let mut planner = Planner::new().with_restored_panes(&ui);
      planner.set_pane_host_width(HOST);

      planner.update(Message::PaneDragStart);
      planner.update(Message::PaneDrag(800.0));
      planner.update(Message::PaneDrag(760.0));
      planner.update(Message::PaneDragEnd);
      let settled = planner.detail_pane_width();
      ui.panes.insert(DETAIL_PANE_KEY.to_owned(), planner.detail_pane_ratio());

      let mut restored = Planner::new().with_restored_panes(&ui);
      restored.set_pane_host_width(HOST);

      assert_eq!(settled, 358.75);
      assert_eq!(restored.detail_pane_width(), settled);
    }
  }

  mod economics {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prices_material_cost_as_the_rolled_up_acquisition_total_plus_sub_build_fees() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      let eco = planner.economics().unwrap();
      let plan = planner.plan().unwrap();
      let acquisition: f64 = plan
        .raw_totals()
        .iter()
        .map(|total| total.qty as f64 * planner.data().price(total.type_id))
        .sum();

      assert!(eco.material_cost > acquisition);
      assert_eq!(eco.material_cost, 121.25);
      assert_eq!(acquisition, 115.0);
      assert_eq!(eco.profit, eco.revenue - eco.material_cost - eco.install_fee);
    }

    #[test]
    fn it_recomputes_revenue_material_cost_and_profit() {
      let planner = planner();

      let eco = planner.economics().unwrap();

      assert_eq!(eco.revenue, 200_000_000.0);
      assert_eq!(eco.material_cost, 60_000_025.0);
      assert_eq!(eco.profit, eco.revenue - eco.material_cost - eco.install_fee);
      assert!(eco.profitable());
    }

    #[test]
    fn it_reports_zero_isk_per_hour_when_build_time_is_zero() {
      let mut data = PlannerData::default();
      data.recipes.insert(
        COMPONENT,
        recipe(COMPONENT + 1, 1, false, vec![Material::new(TRITANIUM, 1)]),
      );
      data.recipes.get_mut(&COMPONENT).unwrap().time_per_run = 0;
      data.prices.insert(COMPONENT, 10.0);
      data.prices.insert(TRITANIUM, 1.0);
      let mut planner = Planner::new();
      planner.apply_data(data);
      planner.update(Message::ProductPicked(COMPONENT));

      let eco = planner.economics().unwrap();

      assert_eq!(eco.isk_per_hour(), 0.0);
    }
  }

  mod segment_build_time {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::industry::planner_loaders::{PlanClone, PlanPilot};

    fn pilot(industry: i64, advanced_industry: i64) -> PlanPilot {
      PlanPilot {
        advanced_industry,
        industry,
        ..PlanPilot::default()
      }
    }

    fn clone_with(manufacturing: f64, reaction: f64) -> PlanClone {
      PlanClone {
        manufacturing_time_bonus: manufacturing,
        reaction_time_bonus: reaction,
        ..PlanClone::default()
      }
    }

    #[test]
    fn it_stacks_te_skills_and_implant_multiplicatively_for_manufacturing() {
      let recipe = recipe(HULK + 1, 1, false, vec![Material::new(TRITANIUM, 1)]);
      let pilot = pilot(5, 5);
      let clone = clone_with(4.0, 0.0);

      let time = super::super::segment_build_time(&recipe, 1, 18, 1.0, Some((&pilot, Some(&clone))));

      assert!((time - 53.5296).abs() < 1e-9, "got {time}");
    }

    #[test]
    fn it_applies_advanced_industry_and_the_reaction_implant_but_not_industry_for_reactions() {
      let recipe = recipe(COMPONENT + 1, 1, true, vec![Material::new(TRITANIUM, 1)]);
      let pilot = pilot(5, 4);
      let clone = clone_with(8.0, 4.0);

      let time = super::super::segment_build_time(&recipe, 1, 20, 1.0, Some((&pilot, Some(&clone))));

      assert!((time - 84.48).abs() < 1e-9, "got {time}");
    }

    #[test]
    fn it_falls_back_to_blueprint_te_only_when_unassigned() {
      let recipe = recipe(HULK + 1, 1, false, vec![Material::new(TRITANIUM, 1)]);

      let assigned = super::super::segment_build_time(&recipe, 2, 20, 1.0, None);
      let baseline = super::super::node_build_time(&recipe, 2, 20, 1.0);

      assert_eq!(assigned, baseline);
      assert_eq!(assigned, 160.0);
    }

    #[test]
    fn it_applies_skills_with_the_active_clone_and_no_implant() {
      let recipe = recipe(HULK + 1, 1, false, vec![Material::new(TRITANIUM, 1)]);
      let pilot = pilot(0, 5);

      let time = super::super::segment_build_time(&recipe, 1, 0, 1.0, Some((&pilot, None)));

      assert!((time - 85.0).abs() < 1e-9, "got {time}");
    }

    #[test]
    fn it_reduces_manufacturing_time_by_the_rig_te_factor() {
      let recipe = recipe(HULK + 1, 1, false, vec![Material::new(TRITANIUM, 1)]);

      let time = super::super::node_build_time(&recipe, 2, 20, 0.9);

      assert!((time - 144.0).abs() < 1e-9, "got {time}");
    }

    #[test]
    fn it_reduces_reaction_time_by_the_rig_te_factor() {
      let recipe = recipe(COMPONENT + 1, 1, true, vec![Material::new(TRITANIUM, 1)]);

      let time = super::super::node_build_time(&recipe, 1, 0, 0.8);

      assert!((time - 80.0).abs() < 1e-9, "got {time}");
    }
  }

  mod estimated_item_value {
    use pretty_assertions::assert_eq;

    use super::*;

    fn priced_data() -> PlannerData {
      let mut data = PlannerData::default();
      data.adjusted_prices.insert(TRITANIUM, 6.0);
      data.adjusted_prices.insert(RETRIEVER, 1_000.0);
      data
    }

    #[test]
    fn it_falls_back_to_the_market_average_when_no_adjusted_price() {
      let mut data = PlannerData::default();
      data.prices.insert(TRITANIUM, 7.0);
      let recipe = recipe(HULK + 1, 1, false, vec![Material::new(TRITANIUM, 100)]);

      assert_eq!(super::estimated_item_value(&data, &recipe, 1), 700.0);
    }

    #[test]
    fn it_ignores_me_and_prices_base_quantities_at_the_adjusted_price() {
      let data = priced_data();
      let recipe = recipe(
        HULK + 1,
        1,
        false,
        vec![Material::new(RETRIEVER, 2), Material::new(TRITANIUM, 50)],
      );

      assert_eq!(super::estimated_item_value(&data, &recipe, 1), 2_300.0);
    }

    #[test]
    fn it_scales_linearly_with_runs() {
      let data = priced_data();
      let recipe = recipe(HULK + 1, 1, false, vec![Material::new(TRITANIUM, 50)]);

      assert_eq!(super::estimated_item_value(&data, &recipe, 4), 1_200.0);
    }

    #[test]
    fn it_prices_a_reaction_at_its_base_quantities() {
      let mut data = priced_data();
      data.adjusted_prices.insert(34_001, 100.0);
      let recipe = recipe(HULK + 1, 40, true, vec![Material::new(34_001, 25)]);

      assert_eq!(super::estimated_item_value(&data, &recipe, 3), 7_500.0);
    }
  }

  mod facility_picker {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_the_popover_for_the_toggled_type() {
      let mut planner = planner();

      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });

      assert_eq!(planner.facility_picker().map(|state| state.type_id), Some(HULK));
    }

    #[test]
    fn it_opens_the_popover_when_typing_opens_the_picker() {
      let mut planner = planner();

      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "cheap".to_owned(),
      });

      assert_eq!(planner.facility_picker().map(|state| state.type_id), Some(HULK));
    }

    #[test]
    fn it_applies_a_picked_facility_to_only_that_types_settings() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      planner.update(Message::FacilitySelected {
        facility_structure: 60_000_001,
        pin: None,
        solar_system_id: 30_000_142,
        type_id: RETRIEVER,
      });

      assert_eq!(planner.settings_for(RETRIEVER).facility_structure, Some(60_000_001));
      assert_eq!(planner.settings_for(RETRIEVER).facility_system, Some(30_000_142));
      assert_eq!(planner.settings_for(HULK).facility_structure, None);
    }

    #[test]
    fn it_applies_live_results_for_the_current_generation() {
      let mut planner = planner();
      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "Jita".to_owned(),
      });
      let generation = planner.facility_picker().unwrap().search_generation;

      planner.update(Message::FacilitySearchResults {
        generation,
        type_id: HULK,
        results: vec![facility(1_021_000_000_001, 30_000_142, "Jita Keepstar", 0.04)],
      });

      let state = planner.facility_picker().unwrap();
      assert_eq!(state.results.len(), 1);
      assert_eq!(state.results[0].name, "Jita Keepstar");
      assert!(!state.searching);
    }

    #[test]
    fn it_bumps_the_search_generation_and_flags_searching_at_three_chars() {
      let mut planner = planner();
      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      let before = planner.facility_picker().unwrap().search_generation;

      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "Jita".to_owned(),
      });

      let state = planner.facility_picker().unwrap();
      assert_eq!(state.search_generation, before + 1);
      assert!(state.searching);
    }

    #[test]
    fn it_defaults_a_node_to_the_cheapest_eligible_facility() {
      let planner = planner();

      let facility = planner.selected_facility(HULK, false).unwrap();

      assert_eq!(facility.name, "Cheap Citadel");
      assert_eq!(facility.solar_system_id, 30_002_187);
    }

    #[test]
    fn it_drops_results_routed_to_a_different_node() {
      let mut planner = planner();
      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "Jita".to_owned(),
      });
      let generation = planner.facility_picker().unwrap().search_generation;

      planner.update(Message::FacilitySearchResults {
        generation,
        type_id: RETRIEVER,
        results: vec![facility(1_021_000_000_001, 30_000_142, "Wrong Node", 0.04)],
      });

      assert!(planner.facility_picker().unwrap().results.is_empty());
    }

    #[test]
    fn it_drops_results_stamped_with_a_stale_generation() {
      let mut planner = planner();
      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "Jita".to_owned(),
      });
      let stale = planner.facility_picker().unwrap().search_generation;
      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "Jitan".to_owned(),
      });

      planner.update(Message::FacilitySearchResults {
        generation: stale,
        type_id: HULK,
        results: vec![facility(1_021_000_000_001, 30_000_142, "Stale Hit", 0.04)],
      });

      assert!(planner.facility_picker().unwrap().results.is_empty());
    }

    #[test]
    fn it_falls_back_to_local_and_clears_results_below_three_chars() {
      let mut planner = planner();
      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "Jita".to_owned(),
      });
      let generation = planner.facility_picker().unwrap().search_generation;
      planner.update(Message::FacilitySearchResults {
        generation,
        type_id: HULK,
        results: vec![facility(1_021_000_000_001, 30_000_142, "Jita Keepstar", 0.04)],
      });

      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "Ji".to_owned(),
      });

      let state = planner.facility_picker().unwrap();
      assert!(state.results.is_empty());
      assert!(!state.searching);
    }

    #[test]
    fn it_only_keeps_one_node_picker_open_at_a_time() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      planner.update(Message::FacilityPickerToggled {
        type_id: RETRIEVER,
      });

      assert_eq!(planner.facility_picker().map(|state| state.type_id), Some(RETRIEVER));
    }

    #[test]
    fn it_opens_a_nodes_picker_when_its_always_visible_field_is_typed_into() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });

      planner.update(Message::FacilitySearchChanged {
        type_id: RETRIEVER,
        query: "amarr".to_owned(),
      });

      assert_eq!(planner.facility_picker().map(|state| state.type_id), Some(RETRIEVER));
      assert_eq!(
        planner.facility_picker().map(|state| state.query.clone()),
        Some("amarr".to_owned())
      );
    }

    #[test]
    fn it_pins_the_selected_facility_system_and_closes_the_picker() {
      let mut planner = planner();
      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });

      planner.update(Message::FacilitySelected {
        type_id: HULK,
        facility_structure: 60_000_001,
        pin: None,
        solar_system_id: 30_000_142,
      });

      assert_eq!(planner.settings_for(HULK).facility_system, Some(30_000_142));
      assert_eq!(planner.settings_for(HULK).facility_structure, Some(60_000_001));
      assert_eq!(planner.selected_facility(HULK, false).unwrap().name, "Pricey Station");
      assert!(planner.facility_picker().is_none());
    }

    #[test]
    fn it_records_the_search_query_for_the_open_node() {
      let mut planner = planner();
      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });

      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "cheap".to_owned(),
      });

      assert_eq!(
        planner.facility_picker().map(|state| state.query.clone()),
        Some("cheap".to_owned())
      );
    }

    #[test]
    fn it_resolves_the_exact_picked_structure_when_a_system_hosts_several() {
      let mut planner = planner();
      planner
        .data
        .facilities
        .push(facility(60_000_003, 30_000_142, "Reaction Array", 0.5));
      planner.update(Message::FacilitySelected {
        facility_structure: 60_000_003,
        pin: None,
        solar_system_id: 30_000_142,
        type_id: HULK,
      });

      let facility = planner.selected_facility(HULK, false).unwrap();

      assert_eq!(facility.id, 60_000_003);
      assert_eq!(facility.name, "Reaction Array");
    }

    #[test]
    fn it_toggles_a_per_node_picker_open_and_closed() {
      let mut planner = planner();

      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      assert_eq!(planner.facility_picker().map(|state| state.type_id), Some(HULK));

      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      assert!(planner.facility_picker().is_none());
    }
  }

  mod fresh_node {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_a_reaction_to_zero_efficiency() {
      let mut data = PlannerData::default();
      data.recipes.insert(
        COMPONENT,
        recipe(COMPONENT + 1, 100, true, vec![Material::new(TRITANIUM, 1)]),
      );
      let mut planner = Planner::new();
      planner.apply_data(data);

      planner.update(Message::ProductPicked(COMPONENT));

      assert_eq!(planner.settings_for(COMPONENT).me, 0);
      assert_eq!(planner.settings_for(COMPONENT).te, 0);
    }

    #[test]
    fn it_seeds_me_and_te_from_the_best_owned_blueprint() {
      let mut planner = planner();
      planner.data.owned.insert(
        HULK,
        OwnedSummary {
          in_scope: true,
          is_original: true,
          material_efficiency: 8,
          time_efficiency: 16,
        },
      );

      planner.update(Message::ProductPicked(HULK));

      assert_eq!(planner.settings_for(HULK).me, 8);
      assert_eq!(planner.settings_for(HULK).te, 16);
    }
  }

  mod install_fee {
    use pretty_assertions::assert_eq;

    #[test]
    fn it_sums_gross_cost_facility_tax_and_scc_surcharge() {
      let fee = super::install_fee(1_000_000.0, 0.05, 1.0);

      assert_eq!(fee, 92_500.0);
    }

    #[test]
    fn it_charges_only_the_flat_fees_at_a_zero_cost_index() {
      let fee = super::install_fee(1_000_000.0, 0.0, 1.0);

      assert_eq!(fee, 42_500.0);
    }

    #[test]
    fn it_matches_an_eve_ref_reference_job_within_rounding() {
      let eiv = 6_147_769_967.0_f64;
      let cost_index = 79_306_233.0 / eiv;

      let fee = super::install_fee_with_facility_tax(eiv, cost_index, 1.0, 0.02);

      assert!((fee - 448_172_431.0).abs() < 1.0);
    }

    #[test]
    fn it_scales_only_the_gross_cost_by_the_rig_fee_factor() {
      let fee = super::install_fee(1_000_000.0, 0.05, 0.9);

      assert_eq!(fee, 87_500.0);
    }
  }

  mod memoization {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_leaves_the_cached_plan_untouched_on_a_cursor_move() {
      let mut planner = planner();
      let before = planner.plan();

      planner.update(Message::CursorMoved(Point::new(10.0, 20.0)));

      assert_eq!(planner.plan(), before);
    }

    #[test]
    fn it_reflects_a_runs_change_in_the_cached_raw_totals() {
      let mut planner = planner();
      let one_run = planner.raw_totals().to_vec();

      planner.update(Message::RunsChanged(3));

      let three_runs = planner.raw_totals();
      assert_eq!(three_runs, planner.plan().unwrap().raw_totals().as_slice());
      assert_ne!(three_runs, one_run.as_slice());
    }

    #[test]
    fn it_refreshes_the_cached_allocation_when_on_hand_loads() {
      let mut planner = planner();
      let site = 60_000_002;
      planner.update(Message::StockSelectionToggled {
        site,
        type_id: TRITANIUM,
      });
      assert_eq!(planner.stock_allocation().drawn_for_type(TRITANIUM), 0);

      planner.set_on_hand(HashMap::from([((site, TRITANIUM), 3)]));

      assert_eq!(planner.stock_allocation().drawn_for_type(TRITANIUM), 3);
    }

    #[test]
    fn it_refreshes_the_cached_plan_after_a_plan_affecting_update() {
      let mut planner = planner();
      let before = planner.merged_build_order().len();

      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      assert!(planner.merged_build_order().len() > before);
      assert_eq!(
        planner.merged_build_order(),
        planner.plan().unwrap().merged_build_order().as_slice()
      );
      assert_eq!(planner.raw_totals(), planner.plan().unwrap().raw_totals().as_slice());
    }
  }

  mod order_segments {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_assigns_a_lone_full_unassigned_segment_by_default() {
      let mut planner = planner();
      planner.update(Message::RunsChanged(10));

      let segments = planner.segments_for(HULK);

      assert_eq!(segments.len(), 1);
      assert_eq!(segments[0].runs, 10);
    }

    #[test]
    fn it_counts_assigned_segments_in_the_order_assignment() {
      let mut planner = planner();
      planner.update(Message::RunsChanged(10));

      let (assigned, total) = planner.order_assignment();

      assert_eq!(assigned, 0);
      assert_eq!(total, 1);
    }

    #[test]
    fn it_keeps_segment_runs_summing_to_the_job_total_after_an_edit() {
      let mut planner = planner();
      planner.update(Message::RunsChanged(10));
      planner.update(Message::OrderJobSplit {
        type_id: HULK,
      });

      planner.update(Message::OrderSegmentRunsChanged {
        index: 0,
        type_id: HULK,
        value: "8".to_owned(),
      });

      let segments = planner.segments_for(HULK);
      assert_eq!(segments[0].runs, 8);
      assert_eq!(segments.iter().map(|segment| segment.runs).sum::<i64>(), 10);
    }

    #[test]
    fn it_merges_a_split_job_back_into_one() {
      let mut planner = planner();
      planner.update(Message::RunsChanged(10));
      planner.update(Message::OrderJobSplit {
        type_id: HULK,
      });

      planner.update(Message::OrderJobMerged {
        type_id: HULK,
      });

      let segments = planner.segments_for(HULK);
      assert_eq!(segments.len(), 1);
      assert_eq!(segments[0].runs, 10);
    }

    #[test]
    fn it_opens_a_split_aware_order_menu_on_a_right_press() {
      let mut planner = planner();
      planner.update(Message::RunsChanged(10));
      planner.update(Message::CursorMoved(Point::new(40.0, 80.0)));

      planner.update(Message::OrderJobRightPressed {
        type_id: HULK,
      });

      let menu = planner.order_menu().unwrap();
      assert_eq!(menu.type_id, HULK);
      assert!(!menu.split);
    }

    #[test]
    fn it_removes_a_segment_and_folds_its_runs_back() {
      let mut planner = planner();
      planner.update(Message::RunsChanged(10));
      planner.update(Message::OrderJobSplit {
        type_id: HULK,
      });

      planner.update(Message::OrderSegmentRemoved {
        index: 1,
        type_id: HULK,
      });

      let segments = planner.segments_for(HULK);
      assert_eq!(segments.len(), 1);
      assert_eq!(segments[0].runs, 10);
    }

    #[test]
    fn it_round_trips_segments_through_export_and_restore() {
      let mut planner = planner();
      planner.update(Message::RunsChanged(10));
      planner.update(Message::OrderJobSplit {
        type_id: HULK,
      });
      let exported = planner.segments();

      let mut reloaded = super::planner();
      reloaded.update(Message::RunsChanged(10));
      reloaded.restore_segments(&exported);

      assert_eq!(reloaded.segments_for(HULK).len(), 2);
    }

    #[test]
    fn it_splits_the_root_job_into_two_segments() {
      let mut planner = planner();
      planner.update(Message::RunsChanged(10));

      planner.update(Message::OrderJobSplit {
        type_id: HULK,
      });

      let segments = planner.segments_for(HULK);
      assert_eq!(segments.len(), 2);
      assert_eq!(segments.iter().map(|segment| segment.runs).sum::<i64>(), 10);
    }
  }

  mod pilot_assignment {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::industry::planner_loaders::{PlanClone, PlanPilot};

    fn pilot(id: i64, name: &str) -> PlanPilot {
      PlanPilot {
        clones: vec![
          PlanClone {
            id: None,
            implant_names: vec!["Zainou 'Beancounter'".to_owned()],
            location: Some("Jita IV - Moon 4".to_owned()),
            name: "Active clone".to_owned(),
            ..PlanClone::default()
          },
          PlanClone {
            id: Some(7),
            implant_names: Vec::new(),
            location: Some("Amarr VIII".to_owned()),
            name: "Industry clone".to_owned(),
            ..PlanClone::default()
          },
        ],
        id,
        name: name.to_owned(),
        portrait: None,
        ..PlanPilot::default()
      }
    }

    fn assignable() -> Planner {
      let mut planner = planner();
      planner.set_assign_pilots(true);
      planner.set_pilots(vec![pilot(1, "Miner Joe")]);
      planner.update(Message::RunsChanged(10));
      planner
    }

    #[test]
    fn it_assigns_a_pilot_and_clone_to_the_lone_segment() {
      let mut planner = assignable();

      planner.update(Message::OrderPilotAssigned {
        clone_id: Some(7),
        index: 0,
        pilot_id: Some(1),
        type_id: HULK,
      });

      let segment = &planner.segments_for(HULK)[0];
      assert_eq!(segment.pilot_id, Some(1));
      assert_eq!(segment.clone_id, Some(7));
      assert_eq!(planner.order_assignment(), (1, 1));
    }

    #[test]
    fn it_assigns_the_active_clone_with_a_null_clone_id() {
      let mut planner = assignable();

      planner.update(Message::OrderPilotAssigned {
        clone_id: None,
        index: 0,
        pilot_id: Some(1),
        type_id: HULK,
      });

      let segment = &planner.segments_for(HULK)[0];
      assert_eq!(segment.pilot_id, Some(1));
      assert_eq!(segment.clone_id, None);
    }

    #[test]
    fn it_unassigns_a_segment() {
      let mut planner = assignable();
      planner.update(Message::OrderPilotAssigned {
        clone_id: Some(7),
        index: 0,
        pilot_id: Some(1),
        type_id: HULK,
      });

      planner.update(Message::OrderPilotAssigned {
        clone_id: None,
        index: 0,
        pilot_id: None,
        type_id: HULK,
      });

      let segment = &planner.segments_for(HULK)[0];
      assert_eq!(segment.pilot_id, None);
      assert_eq!(planner.order_assignment(), (0, 1));
    }

    #[test]
    fn it_keeps_assignment_inert_when_the_feature_is_disabled() {
      let mut planner = planner();
      planner.set_assign_pilots(false);
      planner.set_pilots(vec![pilot(1, "Miner Joe")]);
      planner.update(Message::RunsChanged(10));

      planner.update(Message::OrderPilotAssigned {
        clone_id: Some(7),
        index: 0,
        pilot_id: Some(1),
        type_id: HULK,
      });

      assert!(!planner.assign_pilots());
      assert!(planner.pilots().is_empty());
      assert_eq!(planner.segments_for(HULK)[0].pilot_id, None);
    }

    #[test]
    fn it_drops_the_pool_and_closes_the_picker_when_disabled() {
      let mut planner = assignable();
      planner.update(Message::OrderPilotPickerToggled {
        index: 0,
        type_id: HULK,
      });
      assert!(planner.pilot_picker_open(HULK, 0));

      planner.set_assign_pilots(false);

      assert!(!planner.pilot_picker_open(HULK, 0));
      assert!(planner.pilots().is_empty());
    }

    #[test]
    fn it_resolves_an_assigned_pilots_clone_for_display() {
      let mut planner = assignable();
      planner.update(Message::OrderPilotAssigned {
        clone_id: Some(7),
        index: 0,
        pilot_id: Some(1),
        type_id: HULK,
      });

      let resolved = planner.pilot(1).unwrap();
      assert_eq!(resolved.name, "Miner Joe");
      assert_eq!(resolved.clone_named(Some(7)).unwrap().name, "Industry clone");
    }

    #[test]
    fn it_tracks_the_expanded_pilot_in_the_open_picker() {
      let mut planner = assignable();
      planner.set_pilots(vec![pilot(1, "Miner Joe"), pilot(2, "Hauler Sue")]);
      planner.update(Message::OrderPilotPickerToggled {
        index: 0,
        type_id: HULK,
      });

      planner.update(Message::OrderPilotPickerExpanded {
        index: 0,
        pilot_id: 2,
        type_id: HULK,
      });

      assert_eq!(planner.pilot_picker_expanded(), Some(2));
    }
  }

  mod picker_scroll {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_records_the_absolute_picker_scroll_offset() {
      let mut planner = planner();

      planner.update(Message::PickerScrolled {
        absolute: 640.0,
      });

      assert_eq!(planner.picker_scroll_offset(), 640.0);
    }

    #[test]
    fn it_resets_the_scroll_offset_when_the_category_changes() {
      let mut planner = planner();
      planner.update(Message::PickerScrolled {
        absolute: 640.0,
      });

      planner.update(Message::CategorySelected(Category::Ship));

      assert_eq!(planner.picker_scroll_offset(), 0.0);
    }

    #[test]
    fn it_resets_the_scroll_offset_when_the_search_query_changes() {
      let mut planner = planner();
      planner.update(Message::PickerScrolled {
        absolute: 640.0,
      });

      planner.update(Message::SearchChanged("hulk".to_owned()));

      assert_eq!(planner.picker_scroll_offset(), 0.0);
    }
  }

  mod row_collapse {
    use iced::advanced::widget::Tree;

    use super::*;

    #[test]
    fn it_keeps_a_collapsed_row_built_and_in_the_raw_totals() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      planner.update(Message::RowCollapseToggled {
        type_id: RETRIEVER,
      });

      assert!(planner.is_built(RETRIEVER));
      let totals = planner.plan().unwrap().raw_totals();
      assert!(totals.iter().all(|total| total.type_id != RETRIEVER));
    }

    #[test]
    fn it_renders_a_collapsed_built_row_without_its_children() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      planner.update(Message::RowCollapseToggled {
        type_id: RETRIEVER,
      });

      let _ = Tree::new(super::super::view(&planner, Scope::All).as_widget());
    }

    #[test]
    fn it_resets_collapsed_rows_when_a_new_product_is_picked() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      planner.update(Message::RowCollapseToggled {
        type_id: RETRIEVER,
      });

      planner.update(Message::ProductPicked(RETRIEVER));

      assert!(!planner.is_row_collapsed(RETRIEVER));
    }

    #[test]
    fn it_starts_expanded_for_a_built_row() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      assert!(!planner.is_row_collapsed(RETRIEVER));
    }

    #[test]
    fn it_toggles_a_built_rows_subtree_collapsed_and_expanded() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      planner.update(Message::RowCollapseToggled {
        type_id: RETRIEVER,
      });
      assert!(planner.is_row_collapsed(RETRIEVER));

      planner.update(Message::RowCollapseToggled {
        type_id: RETRIEVER,
      });
      assert!(!planner.is_row_collapsed(RETRIEVER));
    }
  }

  mod runs_changed {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_an_edited_runs_field_to_the_maximum() {
      let mut planner = planner();

      planner.update(Message::RunsInputChanged("99999".to_owned()));

      assert_eq!(planner.runs(), RUNS_MAX);
    }

    #[test]
    fn it_clamps_runs_to_the_valid_range_and_recomputes_output() {
      let mut planner = planner();

      planner.update(Message::RunsChanged(0));
      assert_eq!(planner.runs(), 1);

      planner.update(Message::RunsChanged(50));
      assert_eq!(planner.economics().unwrap().output_qty, 50);
    }

    #[test]
    fn it_holds_at_one_run_for_an_empty_or_zero_runs_field() {
      let mut planner = planner();

      planner.update(Message::RunsInputChanged(String::new()));
      assert_eq!(planner.runs(), 1);
      assert_eq!(planner.runs_input(), "");

      planner.update(Message::RunsInputChanged("0".to_owned()));
      assert_eq!(planner.runs(), 1);
      assert_eq!(planner.runs_input(), "0");
    }

    #[test]
    fn it_keeps_only_digits_and_reflows_from_an_edited_runs_field() {
      let mut planner = planner();

      planner.update(Message::RunsInputChanged("4x2".to_owned()));

      assert_eq!(planner.runs(), 42);
      assert_eq!(planner.runs_input(), "42");
      assert_eq!(planner.economics().unwrap().output_qty, 42);
    }

    #[test]
    fn it_scales_sub_build_runs_with_root_runs() {
      let mut planner = planner();
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        type_id: HULK,
      });
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      planner.update(Message::RunsChanged(3));

      let builds = planner.plan().unwrap().collect_builds();
      assert_eq!(builds[0].runs, 6);
    }

    #[test]
    fn it_syncs_the_runs_field_text_when_steppers_change_the_count() {
      let mut planner = planner();

      planner.update(Message::RunsChanged(7));

      assert_eq!(planner.runs(), 7);
      assert_eq!(planner.runs_input(), "7");
    }
  }

  mod saved_plans {
    use pretty_assertions::assert_eq;

    use super::*;

    fn saved_data(id: i64, name: &str, planner: &Planner) -> SavedPlanData {
      SavedPlanData {
        id,
        name: name.to_owned(),
        tree: planner.snapshot().unwrap(),
      }
    }

    #[test]
    fn it_leaves_economics_unset_when_the_recipe_is_unknown() {
      let source = planner();
      let data = saved_data(1, "Orphan", &source);
      let mut empty = Planner::new();
      empty.apply_data(PlannerData::default());

      empty.update(Message::PlansListed(vec![data]));

      assert!(empty.saved()[0].economics.is_none());
    }

    #[test]
    fn it_recomputes_economics_for_each_listed_plan_at_current_prices() {
      let mut planner = planner();
      let data = saved_data(1, "Hulk run", &planner);

      planner.update(Message::PlansListed(vec![data]));

      let saved = planner.saved();
      assert_eq!(saved.len(), 1);
      assert_eq!(saved[0].name, "Hulk run");
      assert_eq!(saved[0].product_type_id, HULK);
      let eco = saved[0].economics.as_ref().unwrap();
      assert_eq!(eco.revenue, 200_000_000.0);
      assert_eq!(eco.material_cost, planner.economics().unwrap().material_cost);
    }

    #[test]
    fn it_rehydrates_the_tree_and_returns_to_the_detail_tab_on_restore() {
      let mut source = planner();
      source.update(Message::RunsChanged(4));
      source.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      let tree = source.snapshot().unwrap();

      let mut planner = planner();
      planner.update(Message::RightTabSelected(RightTab::Plans));
      planner.update(Message::PlanRestored {
        segments: Vec::new(),
        tree: Box::new(tree),
      });

      assert_eq!(planner.runs(), 4);
      assert_eq!(planner.right_tab(), RightTab::Detail);
      assert_eq!(planner.plan(), source.plan());
    }
  }

  mod search_placeholder {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_when_no_catalog_is_loaded() {
      let planner = Planner::new();

      assert_eq!(planner.search_placeholder(), "Search buildable products\u{2026}");
    }

    #[test]
    fn it_reports_the_buildable_catalog_size() {
      let planner = planner();

      assert_eq!(planner.search_placeholder(), "Search 1 buildable products\u{2026}");
    }
  }

  mod seed_from_blueprint {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_applies_a_queued_seed_once_data_loads() {
      let mut planner = Planner::new();
      planner.queue_blueprint_seed(HULK + 1);

      let mut data = PlannerData::default();
      data
        .recipes
        .insert(HULK, recipe(HULK + 1, 1, false, vec![Material::new(TRITANIUM, 5)]));
      data.names.insert(HULK, "Hulk".to_owned());
      data.catalog.push(CatalogEntry {
        category: Category::Ship,
        group_name: "Mining Barge".to_owned(),
        is_reaction: false,
        name: "Hulk".to_owned(),
        type_id: HULK,
        volume: 3_750.0,
      });
      planner.apply_data(data);

      assert_eq!(planner.product(), Some(HULK));
    }

    #[test]
    fn it_reports_no_seed_for_an_unknown_blueprint() {
      let mut planner = planner();

      assert!(!planner.seed_from_blueprint(123_456));
    }

    #[test]
    fn it_seeds_the_product_a_blueprint_makes() {
      let mut planner = planner();
      planner.update(Message::ProductPicked(RETRIEVER));

      let seeded = planner.seed_from_blueprint(HULK + 1);

      assert!(seeded);
      assert_eq!(planner.product(), Some(HULK));
      assert!(!planner.is_built(RETRIEVER));
    }
  }

  mod select_product {
    use pretty_assertions::assert_eq;

    use super::*;

    fn planner_with_defaults(defaults: FacilityDefaults) -> Planner {
      const SULFURIC: i64 = 16_011;
      let mut data = PlannerData::default();
      data
        .recipes
        .insert(HULK, recipe(HULK + 1, 1, false, vec![Material::new(TRITANIUM, 5)]));
      data.recipes.insert(
        SULFURIC,
        recipe(SULFURIC + 1, 100, true, vec![Material::new(TRITANIUM, 5)]),
      );
      data.names.insert(HULK, "Hulk".to_owned());
      data.names.insert(SULFURIC, "Sulfuric Acid".to_owned());
      data.facilities = vec![
        facility(60_000_002, 30_002_187, "Manufacturing Hub", 0.02),
        facility(1_021_000_000_009, 30_000_142, "Reaction Fortizar", 0.03),
      ];

      let mut planner = Planner::new();
      planner.set_facility_defaults(defaults);
      planner.apply_data(data);
      planner
    }

    #[test]
    fn it_leaves_the_root_facility_unset_when_no_default_is_configured() {
      let mut planner = planner_with_defaults(FacilityDefaults::default());

      planner.update(Message::ProductPicked(HULK));

      assert_eq!(planner.settings_for(HULK).facility_system, None);
    }

    #[test]
    fn it_leaves_the_root_facility_unset_when_the_default_is_absent_from_data() {
      let mut planner = planner_with_defaults(FacilityDefaults {
        manufacturing: Some(70_000_000),
        reactions: None,
      });

      planner.update(Message::ProductPicked(HULK));

      assert_eq!(planner.settings_for(HULK).facility_system, None);
    }

    #[test]
    fn it_seeds_a_reaction_with_the_reactions_default_and_manufacturing_with_its_own() {
      const SULFURIC: i64 = 16_011;
      let mut reaction_planner = planner_with_defaults(FacilityDefaults {
        manufacturing: Some(60_000_002),
        reactions: Some(1_021_000_000_009),
      });
      let mut manufacturing_planner = planner_with_defaults(FacilityDefaults {
        manufacturing: Some(60_000_002),
        reactions: Some(1_021_000_000_009),
      });

      reaction_planner.update(Message::ProductPicked(SULFURIC));
      manufacturing_planner.update(Message::ProductPicked(HULK));

      assert_eq!(
        reaction_planner.settings_for(SULFURIC).facility_structure,
        Some(1_021_000_000_009)
      );
      assert_eq!(
        manufacturing_planner.settings_for(HULK).facility_structure,
        Some(60_000_002)
      );
    }

    #[test]
    fn it_seeds_every_sub_build_from_the_per_activity_default() {
      const WIDGET: i64 = 50_001;
      const REACTED: i64 = 50_002;
      let mut data = PlannerData::default();
      data
        .recipes
        .insert(WIDGET, recipe(WIDGET + 1, 1, false, vec![Material::new(REACTED, 10)]));
      data.recipes.insert(
        REACTED,
        recipe(REACTED + 1, 100, true, vec![Material::new(TRITANIUM, 5)]),
      );
      data.names.insert(WIDGET, "Widget".to_owned());
      data.names.insert(REACTED, "Reacted Goo".to_owned());
      data.facilities = vec![
        facility(60_000_002, 30_002_187, "Manufacturing Hub", 0.02),
        facility(60_000_003, 30_000_142, "Reaction Hub", 0.03),
      ];
      let mut planner = Planner::new();
      planner.set_facility_defaults(FacilityDefaults {
        manufacturing: Some(60_000_002),
        reactions: Some(60_000_003),
      });
      planner.apply_data(data);

      planner.update(Message::ProductPicked(WIDGET));
      planner.update(Message::NodeBrokenDown {
        type_id: REACTED,
      });

      assert_eq!(
        planner.settings_for(WIDGET).facility_system,
        Some(30_002_187),
        "the root manufacturing job seeds the manufacturing default"
      );
      assert_eq!(
        planner.settings_for(REACTED).facility_system,
        Some(30_000_142),
        "the reaction sub-build seeds the reaction default, not the cheapest fallback"
      );
    }

    #[test]
    fn it_seeds_the_root_facility_from_the_manufacturing_default() {
      let mut planner = planner_with_defaults(FacilityDefaults {
        manufacturing: Some(60_000_002),
        reactions: None,
      });

      planner.update(Message::ProductPicked(HULK));

      assert_eq!(planner.settings_for(HULK).facility_system, Some(30_002_187));
    }

    #[test]
    fn it_seeds_the_root_facility_from_the_reaction_default_for_a_reaction_product() {
      const SULFURIC: i64 = 16_011;
      let mut planner = planner_with_defaults(FacilityDefaults {
        manufacturing: Some(60_000_002),
        reactions: Some(1_021_000_000_009),
      });

      planner.update(Message::ProductPicked(SULFURIC));

      assert_eq!(planner.settings_for(SULFURIC).facility_system, Some(30_000_142));
    }
  }

  mod shopping_list {
    use super::*;

    #[test]
    fn it_lists_rolled_up_raw_inputs_quantity_first() {
      let mut planner = planner();
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        type_id: HULK,
      });
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        type_id: RETRIEVER,
      });

      let list = planner.shopping_list();

      assert!(list.contains("25\tTritanium"));
      assert!(!list.contains("Retriever"));
    }
  }

  mod snapshot {
    use pretty_assertions::assert_eq;

    use super::*;

    fn configured_planner() -> Planner {
      let mut planner = planner();
      planner.update(Message::RunsChanged(3));
      planner.update(Message::MaterialEfficiencyChanged {
        me: 7,
        type_id: HULK,
      });
      planner.update(Message::TimeEfficiencyChanged {
        type_id: HULK,
        te: 14,
      });
      planner.update(Message::FacilitySelected {
        type_id: HULK,
        facility_structure: 60_000_001,
        pin: None,
        solar_system_id: 30_000_142,
      });
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      planner.update(Message::MaterialEfficiencyChanged {
        me: 4,
        type_id: RETRIEVER,
      });
      planner.update(Message::FacilitySelected {
        type_id: RETRIEVER,
        facility_structure: 60_000_002,
        pin: None,
        solar_system_id: 30_002_187,
      });
      planner
    }

    #[test]
    fn it_captures_the_root_and_per_type_configuration() {
      let snapshot = configured_planner().snapshot().unwrap();

      assert_eq!(snapshot.product_type_id, HULK);
      assert_eq!(snapshot.runs, 3);
      assert_eq!(snapshot.root_facility_system, Some(30_000_142));

      let root = snapshot.types.iter().find(|kind| kind.type_id == HULK).unwrap();
      assert_eq!((root.me, root.te, root.built), (7, 14, false));
      assert_eq!(root.facility_structure, Some(60_000_001));

      let child = snapshot.types.iter().find(|kind| kind.type_id == RETRIEVER).unwrap();
      assert_eq!(
        (child.me, child.facility_system, child.facility_structure, child.built),
        (4, Some(30_002_187), Some(60_000_002), true)
      );
    }

    #[test]
    fn it_captures_the_use_stock_intent_per_type() {
      let mut planner = configured_planner();
      planner.update(Message::StockSelectionToggled {
        site: 60_000_001,
        type_id: TRITANIUM,
      });

      let snapshot = planner.snapshot().unwrap();

      let trit = snapshot.types.iter().find(|kind| kind.type_id == TRITANIUM).unwrap();
      let hulk = snapshot.types.iter().find(|kind| kind.type_id == HULK).unwrap();
      assert!(trit.use_stock);
      assert!(!hulk.use_stock);
    }

    #[test]
    fn it_rebuilds_the_use_stock_selection_from_the_saved_facility_structure() {
      let mut planner = configured_planner();
      planner.update(Message::StockSelectionToggled {
        site: 60_000_001,
        type_id: TRITANIUM,
      });
      let snapshot = planner.snapshot().unwrap();

      let mut restored = self::planner();
      restored.restore(&snapshot);

      assert!(restored.is_stock_selected(60_000_001, TRITANIUM));
    }

    #[test]
    fn it_returns_none_without_a_selected_product() {
      let planner = Planner::new();

      assert_eq!(planner.snapshot(), None);
    }

    #[test]
    fn it_round_trips_through_snapshot_and_restore() {
      let original = configured_planner();
      let snapshot = original.snapshot().unwrap();

      let mut restored = planner();
      restored.restore(&snapshot);

      assert_eq!(restored.snapshot(), Some(snapshot));
      assert_eq!(restored.plan(), original.plan());
    }

    #[test]
    fn it_threads_the_picked_structure_onto_the_derived_tree() {
      let planner = configured_planner();

      let plan = planner.plan().unwrap();

      assert_eq!(plan.root.facility_structure, Some(60_000_001));
      let retriever = plan.root.children.get(&RETRIEVER).unwrap();
      assert_eq!(retriever.facility_structure, Some(60_000_002));
    }
  }

  mod stock_selection {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    const SITE: i64 = 60_000_001;

    const SITE_SYSTEM: i64 = 30_000_142;

    fn sited_planner(on_hand: HashMap<(i64, i64), i64>) -> Planner {
      let mut planner = planner();
      planner.update(Message::FacilitySelected {
        facility_structure: SITE,
        pin: None,
        solar_system_id: SITE_SYSTEM,
        type_id: HULK,
      });
      planner.set_on_hand(on_hand);
      planner
    }

    #[test]
    fn it_composes_stock_with_a_breakdown_on_the_remainder() {
      let mut planner = sited_planner(HashMap::from([((SITE, TRITANIUM), 6)]));
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        type_id: HULK,
      });
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        type_id: RETRIEVER,
      });
      planner.update(Message::StockSelectionToggled {
        site: SITE,
        type_id: TRITANIUM,
      });

      let netted = planner
        .plan()
        .unwrap()
        .raw_totals_after_stock(&planner.stock_allocation());

      let trit = netted.iter().find(|total| total.type_id == TRITANIUM).unwrap();
      assert_eq!(trit.qty, 19);
    }

    #[test]
    fn it_drains_the_pool_when_a_material_is_opted_into_stock() {
      let mut planner = sited_planner(HashMap::from([((SITE, TRITANIUM), 4)]));

      planner.update(Message::StockSelectionToggled {
        site: SITE,
        type_id: TRITANIUM,
      });

      assert_eq!(planner.remaining_pool(SITE, TRITANIUM), 0);
      assert_eq!(planner.stock_allocation().drawn_for_type(TRITANIUM), 4);
    }

    #[test]
    fn it_hides_the_button_for_a_later_consumer_once_the_shared_pool_is_drained() {
      let mut planner = sited_planner(HashMap::from([((SITE, TRITANIUM), 6)]));
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      assert!(planner.remaining_pool(SITE, TRITANIUM) > 0);

      planner.update(Message::StockSelectionToggled {
        site: SITE,
        type_id: TRITANIUM,
      });

      assert_eq!(planner.remaining_pool(SITE, TRITANIUM), 0);
    }

    #[test]
    fn it_nets_drawn_stock_off_the_bill_of_materials() {
      let mut planner = sited_planner(HashMap::from([((SITE, TRITANIUM), 4)]));
      planner.update(Message::StockSelectionToggled {
        site: SITE,
        type_id: TRITANIUM,
      });

      let netted = planner
        .plan()
        .unwrap()
        .raw_totals_after_stock(&planner.stock_allocation());

      let trit = netted.iter().find(|total| total.type_id == TRITANIUM).unwrap();
      assert_eq!(trit.qty, 1);
    }

    #[test]
    fn it_recomputes_drawn_stock_live_after_a_snapshot_restore() {
      let mut planner = sited_planner(HashMap::from([((SITE, TRITANIUM), 4)]));
      planner.update(Message::StockSelectionToggled {
        site: SITE,
        type_id: TRITANIUM,
      });
      let snapshot = planner.snapshot().unwrap();

      let mut restored = self::planner();
      restored.restore(&snapshot);
      restored.set_on_hand(HashMap::from([((SITE, TRITANIUM), 4)]));

      assert!(restored.is_stock_selected(SITE, TRITANIUM));
      assert_eq!(restored.stock_allocation().drawn_for_type(TRITANIUM), 4);
    }

    #[test]
    fn it_reports_the_picked_build_site() {
      let planner = sited_planner(HashMap::new());

      assert_eq!(planner.build_sites(), vec![SITE]);
    }

    #[test]
    fn it_shows_a_remaining_pool_for_a_material_with_site_stock() {
      let planner = sited_planner(HashMap::from([((SITE, TRITANIUM), 4)]));

      assert_eq!(planner.remaining_pool(SITE, TRITANIUM), 4);
      assert!(!planner.is_stock_selected(SITE, TRITANIUM));
    }

    #[test]
    fn it_shows_no_pool_for_a_material_without_site_stock() {
      let planner = sited_planner(HashMap::new());

      assert_eq!(planner.remaining_pool(SITE, TRITANIUM), 0);
    }

    #[test]
    fn it_toggling_adds_then_removes_the_selection() {
      let mut planner = sited_planner(HashMap::from([((SITE, TRITANIUM), 4)]));

      planner.update(Message::StockSelectionToggled {
        site: SITE,
        type_id: TRITANIUM,
      });
      assert!(planner.is_stock_selected(SITE, TRITANIUM));

      planner.update(Message::StockSelectionToggled {
        site: SITE,
        type_id: TRITANIUM,
      });
      assert!(!planner.is_stock_selected(SITE, TRITANIUM));
    }
  }

  mod view {
    use iced::advanced::widget::Tree;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_keeps_a_stable_root_tree_shape_whether_or_not_an_overlay_is_open() {
      let mut planner = planner();
      let idle_children = {
        let idle = super::super::view(&planner, Scope::All);
        Tree::new(idle.as_widget()).children.len()
      };

      planner.update(Message::CursorMoved(iced::Point::new(20.0, 40.0)));
      planner.update(Message::MaterialRightPressed {
        type_id: RETRIEVER,
      });

      assert!(planner.menu().is_some());
      let with_menu = super::super::view(&planner, Scope::All);
      assert_eq!(Tree::new(with_menu.as_widget()).children.len(), idle_children);
    }

    #[test]
    fn it_renders_the_facility_picker_panel_across_match_states() {
      let mut root = planner();
      root.update(Message::CursorMoved(Point::new(120.0, 80.0)));
      root.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      let _ = Tree::new(super::super::view(&root, Scope::All).as_widget());

      root.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "zzzznomatch".to_owned(),
      });
      let _ = Tree::new(super::super::view(&root, Scope::All).as_widget());

      let mut nested = planner();
      nested.update(Message::CursorMoved(Point::new(120.0, 80.0)));
      nested.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      nested.update(Message::FacilityPickerToggled {
        type_id: RETRIEVER,
      });
      let _ = Tree::new(super::super::view(&nested, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_material_plan_grid_across_raw_and_buildable_rows() {
      let planner = planner();

      let _ = Tree::new(super::super::view(&planner, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_material_plan_with_a_nested_building_row() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      let _ = Tree::new(super::super::view(&planner, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_material_plan_with_stock_affordances_and_a_used_chip() {
      const SITE: i64 = 60_000_001;
      const SITE_SYSTEM: i64 = 30_000_142;

      let mut planner = planner();
      planner.update(Message::FacilitySelected {
        facility_structure: SITE,
        pin: None,
        solar_system_id: SITE_SYSTEM,
        type_id: HULK,
      });
      planner.set_on_hand(std::collections::HashMap::from([((SITE, TRITANIUM), 4)]));
      let _ = Tree::new(super::super::view(&planner, Scope::All).as_widget());

      planner.update(Message::StockSelectionToggled {
        site: SITE,
        type_id: TRITANIUM,
      });
      assert!(planner.is_stock_selected(SITE, TRITANIUM));

      let _ = Tree::new(super::super::view(&planner, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_merged_build_order_with_a_multi_consumer_subline() {
      const SHIP: i64 = 90_000;
      const LEFT: i64 = 90_001;
      const RIGHT: i64 = 90_002;
      const PLATE: i64 = 90_003;
      const ORE: i64 = 90_004;

      let mut data = PlannerData::default();
      data.recipes.insert(
        SHIP,
        recipe(
          SHIP + 1,
          1,
          false,
          vec![Material::new(LEFT, 1), Material::new(RIGHT, 1)],
        ),
      );
      data
        .recipes
        .insert(LEFT, recipe(LEFT + 1, 1, false, vec![Material::new(PLATE, 3)]));
      data
        .recipes
        .insert(RIGHT, recipe(RIGHT + 1, 1, false, vec![Material::new(PLATE, 4)]));
      data
        .recipes
        .insert(PLATE, recipe(PLATE + 1, 1, false, vec![Material::new(ORE, 5)]));
      for (id, name) in [
        (SHIP, "Ship"),
        (LEFT, "Left"),
        (RIGHT, "Right"),
        (PLATE, "Plate"),
        (ORE, "Ore"),
      ] {
        data.names.insert(id, name.to_owned());
        data.prices.insert(id, 1.0);
      }
      data.catalog.push(CatalogEntry {
        category: Category::Ship,
        group_name: "Test".to_owned(),
        is_reaction: false,
        name: "Ship".to_owned(),
        type_id: SHIP,
        volume: 1.0,
      });

      let mut planner = Planner::new();
      planner.apply_data(data);
      planner.update(Message::ProductPicked(SHIP));
      planner.update(Message::RunsChanged(1));
      planner.update(Message::BreakDownAll);

      let merged = planner.plan().unwrap().merged_build_order();
      let plate = merged.iter().find(|job| job.type_id == PLATE).unwrap();
      assert_eq!(plate.consumers.len(), 2);

      let _ = Tree::new(super::super::view(&planner, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_needed_blueprints_section_across_owned_and_missing_states() {
      let mut missing = planner();
      missing.update(Message::BreakDownAll);
      let _ = Tree::new(super::super::view(&missing, Scope::All).as_widget());

      let mut owned = planner();
      owned.update(Message::BreakDownAll);
      owned.data.owned.insert(
        RETRIEVER,
        OwnedSummary {
          in_scope: true,
          is_original: false,
          material_efficiency: 8,
          time_efficiency: 16,
        },
      );
      owned.data.owned.insert(
        HULK,
        OwnedSummary {
          in_scope: false,
          is_original: true,
          material_efficiency: 0,
          time_efficiency: 0,
        },
      );
      let _ = Tree::new(super::super::view(&owned, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_product_picker_across_empty_and_populated_result_states() {
      let mut populated = planner();
      populated.update(Message::PickerToggled);
      let _ = Tree::new(super::super::view(&populated, Scope::All).as_widget());

      let mut no_match = planner();
      no_match.update(Message::SearchChanged("zzzznomatch".to_owned()));
      let _ = Tree::new(super::super::view(&no_match, Scope::All).as_widget());

      let mut empty = Planner::new();
      empty.update(Message::PickerToggled);
      let _ = Tree::new(super::super::view(&empty, Scope::All).as_widget());

      let mut recent = planner();
      recent.update(Message::PickerToggled);
      recent.update(Message::CategorySelected(Category::Module));
      let _ = Tree::new(super::super::view(&recent, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_product_picker_windowed_to_a_deep_scroll_offset() {
      let mut planner = planner();
      for type_id in 1_000..1_500 {
        planner.data.catalog.push(CatalogEntry {
          category: Category::Module,
          group_name: "Filler".to_owned(),
          is_reaction: false,
          name: format!("Filler {type_id}"),
          type_id,
          volume: 1.0,
        });
        planner.data.prices.insert(type_id, 1.0);
        planner.data.names.insert(type_id, format!("Filler {type_id}"));
      }
      planner.update(Message::SearchChanged("filler".to_owned()));
      planner.update(Message::PickerScrolled {
        absolute: 4_000.0,
      });

      let rendered = super::super::view(&planner, Scope::All);
      let _ = Tree::new(rendered.as_widget());
      assert_eq!(planner.picker_scroll_offset(), 4_000.0);
    }
  }
}
