use std::collections::{BTreeMap, BTreeSet, HashMap};

use iced::Point;

use super::{
  Scope,
  planner_loaders::{self, Category, PlannerData, PlannerFacility, Recipe},
  planner_model::{BuildNode, BuildPlan, MergedBuildJob, RawTotal, StockAllocation, StockSelection, allocate_stock},
};
use crate::{
  store::repo::industry::{PlanTree, PlanType},
  ui::components::resizable_pane::PaneDrag,
  window_state::UiState,
};

pub const DETAIL_PANE_KEY: &str = "industry.planner.detail";

/// Minimum characters before the always-visible facility field triggers a live ESI search; shorter
/// queries fall back to filtering the local `accessible_facilities` list.
pub const FACILITY_SEARCH_MIN_CHARS: usize = 3;

const DEFAULT_ME: i64 = 10;
const DEFAULT_TE: i64 = 20;
const DETAIL_PANE_DEFAULT_WIDTH: f32 = 340.0;
const DETAIL_PANE_MIN_WIDTH: f32 = 280.0;
const INSTALL_FEE_RATE: f64 = 0.5;
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
  pub anchor: Point,
  pub query: String,
  /// Live ESI search results (stations resolved from the SDE, structures via the authenticated
  /// endpoint), replacing the local list once a query reaches [`FACILITY_SEARCH_MIN_CHARS`].
  pub results: Vec<PlannerFacility>,
  /// Bumped on every keystroke; results stamped with an older generation are dropped on receipt so a
  /// slow ESI response cannot overwrite a newer query's results.
  pub search_generation: u64,
  pub searching: bool,
  /// The item type whose facility is being picked (the root product or a built sub-type).
  pub type_id: i64,
}

/// A live-searched structure the planner chose to pin. Carried on [`Message::FacilitySelected`] so the
/// app layer can persist it via the storage `pin_structure` fn; `None` for facilities already known to
/// the SDE (NPC stations) or to corp sync.
#[derive(Clone, Debug, PartialEq)]
pub struct PinnedStructure {
  pub id: i64,
  pub name: String,
  pub solar_system_id: i64,
  pub type_id: Option<i64>,
}

/// The configured default install facilities (station or structure ids) per activity, threaded in from
/// `config::IndustryConfig`. A `None` activity preserves the cheapest-cost-index fallback. Applied when a
/// product is selected, not live-synced into an already-open planner.
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
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart,
  PickerScrolled {
    absolute: f32,
  },
  PickerToggled,
  PlanDeleteRequested(i64),
  PlanLoadRequested(i64),
  PlanRestored(Box<PlanTree>),
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

/// Per-TYPE build settings: the material-efficiency, time-efficiency, and install facility chosen for one
/// item type. Keyed by `type_id` in [`Planner::settings`] (the root product included), so editing a type
/// applies to every occurrence of it in the derived build tree. Whether a type is produced in-house lives
/// separately in [`Planner::built`], not here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TypeSettings {
  /// The picked install structure/station id (the build site). `None` until a facility is picked (the
  /// cheapest-index default carries no structure id); persisted so a later allocation pass can key a
  /// per-site material pool. `facility_system` is its solar system.
  pub facility_structure: Option<i64>,
  pub facility_system: Option<i64>,
  pub me: i64,
  pub te: i64,
}

/// The derived build plan, memoized once per plan-affecting change instead of recomputed every render.
/// iced re-runs `view` on every message/frame, so recomputing the recursive tree (and its deep-cloning
/// roll-ups) per render janks the planner; [`Planner::recompute`] refreshes this whenever a plan input
/// (product/runs/ME/TE/facility/built/stock) changes and `view` reads it.
#[derive(Debug, Default)]
struct Derived {
  allocation: StockAllocation,
  economics: Option<Economics>,
  merged: Vec<MergedBuildJob>,
  plan: Option<BuildPlan>,
  /// `plan.raw_totals()` before stock netting, the input both the bill of materials and `raw_demand_for`
  /// read. Cached so neither re-walks the tree per row.
  raw_totals: Vec<RawTotal>,
}

#[derive(Debug)]
pub struct Planner {
  /// Reverse index from a recipe's `blueprint_type_id` to the product type it makes, built once in
  /// [`apply_data`] from the recipe table (whose keys are product type ids). Turns `runs_me`'s former
  /// linear scan of the whole recipe map into an O(1) lookup.
  bp_to_product: HashMap<i64, i64>,
  /// The set of item types the user chose to produce in-house. The derived build tree descends into a
  /// material only when its type is in this set; everything else is bought.
  built: BTreeSet<i64>,
  category: Category,
  /// Built type ids whose nested material-plan subtree the user collapsed. Keyed per-TYPE (a type appears at
  /// most once in the table). Absent means expanded — the table shows every breakdown by default.
  collapsed_rows: BTreeSet<i64>,
  cursor: Option<Point>,
  data: PlannerData,
  derived: Derived,
  detail_pane: PaneDrag,
  /// Set when an `update` arm changes a plan input; drained at the end of `update` to recompute
  /// [`derived`] exactly once, mirroring the app-layer dirty-flag coalescing discipline.
  dirty: bool,
  facility_defaults: FacilityDefaults,
  facility_picker: Option<FacilityPickerState>,
  loaded: bool,
  menu: Option<MaterialMenu>,
  /// On-hand quantity at the build sites keyed by `(site, type_id)`, the input to [`allocate_stock`]. Loaded
  /// from `store::repo::assets::on_hand_at_build_sites`; empty until that load lands.
  on_hand: HashMap<(i64, i64), i64>,
  /// A blueprint type id queued by "Plan Build" before the catalog finished loading; consumed by
  /// [`Planner::apply_data`] to seed its product as the root once recipes are available.
  pending_blueprint_seed: Option<i64>,
  picker_open: bool,
  picker_scroll_offset: f32,
  placeholder: String,
  product: Option<i64>,
  recent: Vec<i64>,
  right_tab: RightTab,
  runs: i64,
  runs_input: String,
  saved: Vec<SavedPlan>,
  search: String,
  /// Per-TYPE ME/TE/facility, keyed by `type_id` (the root product included). Editing a type here applies
  /// to every occurrence of it in the derived build tree.
  settings: BTreeMap<i64, TypeSettings>,
  /// The ORDERED list of jobs the user opted to draw from on-hand stock. Order is load-bearing:
  /// [`allocate_stock`] drains each shared `(site, type_id)` pool in this order so no unit is double-counted.
  /// A later UI task appends to this; the netting reads it through [`Planner::stock_allocation`].
  stock_selections: Vec<StockSelection>,
}

impl Planner {
  pub fn new() -> Self {
    Planner {
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
      facility_picker: None,
      loaded: false,
      menu: None,
      on_hand: HashMap::new(),
      pending_blueprint_seed: None,
      picker_open: false,
      picker_scroll_offset: 0.0,
      placeholder: String::new(),
      product: None,
      recent: Vec::new(),
      right_tab: RightTab::default(),
      runs: 1,
      runs_input: "1".to_owned(),
      saved: Vec::new(),
      search: String::new(),
      settings: BTreeMap::new(),
      stock_selections: Vec::new(),
    }
  }

  /// Whether `message` changes a plan input (product, runs, ME/TE, facility, breakdown, or stock), so the
  /// memoized [`Derived`] plan must be rebuilt. Excludes `PlanRestored`, whose `restore` handler recomputes
  /// itself. Pure navigation/UI arms (cursor, search, picker, pane, tab) leave the plan untouched.
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
    self.placeholder = format!(
      "Search {} buildable products\u{2026}",
      view::fmt_num(self.data.catalog.len() as i64)
    );
    if self.recent.is_empty() {
      self.recent = self.seed_recent();
    }
    // Cold-open lands on the empty no-product state; only a "Plan Build" queued before the catalog
    // loaded (or a restored plan) pre-populates a product. `seed_recent` still feeds the picker list.
    if let Some(blueprint_type_id) = self.pending_blueprint_seed.take() {
      self.seed_from_blueprint(blueprint_type_id);
    }
    self.recompute();
  }

  /// The distinct picked build-site structure/station ids across the current plan: the root product plus every
  /// configured type that has a facility pinned. The input to `on_hand_at_build_sites`, whose result then feeds
  /// [`set_on_hand`]. Empty when no product is chosen or no facility has been picked yet.
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

  /// Whole-plan economics from the freshly memoized `plan`: `material_cost` is the rolled-up acquisition
  /// total of every raw input the bill of materials says you must buy, plus the install fees of any in-house
  /// sub-builds. With no component broken down this equals pricing the root recipe's materials; once a
  /// component is built in-house it equals buying that component's constituent parts plus its sub-job fee.
  /// Net profit and margin derive from this true cost so they match the bill of materials. Called from
  /// [`recompute`], which reuses the plan it already assembled.
  fn compute_economics(&self, plan: &BuildPlan) -> Option<Economics> {
    let product = self.product?;
    let recipe = self.data.recipe(product)?;

    let material_cost = self.plan_material_cost(plan, &|type_id| self.cost_index(type_id).unwrap_or(0.0));

    let output_qty = recipe.output_per_run * self.runs;
    let revenue = self.data.price(product) * output_qty as f64;
    let install_fee = revenue * self.cost_index(product).unwrap_or(0.0) * INSTALL_FEE_RATE;
    let profit = revenue - material_cost - install_fee;
    let margin = if revenue > 0.0 { profit / revenue * 100.0 } else { 0.0 };
    let per_unit = if output_qty > 0 {
      profit / output_qty as f64
    } else {
      0.0
    };
    let build_time_secs = node_build_time(recipe, self.runs, self.settings_for(product).te);

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

  /// Whether the user collapsed `type_id`'s nested material-plan subtree (hiding the rows it builds from).
  pub fn is_row_collapsed(&self, type_id: i64) -> bool {
    self.collapsed_rows.contains(&type_id)
  }

  pub fn menu(&self) -> Option<&MaterialMenu> {
    self.menu.as_ref()
  }

  /// Whether a memoized plan is present (a product is chosen and its tree assembled). Lets `view` gate the
  /// bill of materials / build order without cloning the cached [`BuildPlan`].
  pub fn has_plan(&self) -> bool {
    self.derived.plan.is_some()
  }

  /// The memoized merged build order (one row per `(type, ME, TE, facility)`), producer-before-consumer.
  /// Recomputed only on a plan-affecting change, so `view` reads it without re-walking the tree.
  pub fn merged_build_order(&self) -> &[MergedBuildJob] {
    &self.derived.merged
  }

  /// The memoized raw totals before stock netting (every raw input the bill of materials must acquire).
  pub fn raw_totals(&self) -> &[RawTotal] {
    &self.derived.raw_totals
  }

  /// The configured (or fresh-default) settings for `type_id`. Per-TYPE: the same settings back every
  /// occurrence of the type in the derived build tree.
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

  /// Whether the current root product has at least one buildable input (a material that itself has a
  /// recipe). Gates the "Break down all" affordance — there is nothing to expand otherwise.
  pub fn has_buildable_inputs(&self) -> bool {
    self
      .product
      .map(|product| !buildable_inputs(&self.data, product).is_empty())
      .unwrap_or(false)
  }

  /// Resolves the product a blueprint type makes (via the loaded recipe table) and seeds it as the planner
  /// root with a fresh breakdown tree. Returns whether a product was found and seeded. The reseed is silent:
  /// any in-progress tree is discarded.
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

  /// Queues a blueprint type to seed as the planner root once the catalog finishes loading. Used by
  /// "Plan Build" when the Planner tab has not loaded its data yet; [`Planner::apply_data`] consumes it.
  pub fn queue_blueprint_seed(&mut self, blueprint_type_id: i64) {
    self.pending_blueprint_seed = Some(blueprint_type_id);
  }

  /// The product type id a blueprint type manufactures (or reacts into), looked up in the loaded recipe
  /// table whose keys are product type ids carrying their producing `blueprint_type_id`.
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
    // Refresh the memoized plan from the restored product/settings/built before rehydrating stock intent:
    // `restore_stock_selections` reads each type's raw demand from the cached pre-netting totals.
    self.recompute();
    self.restore_stock_selections(tree);
    self.facility_picker = None;
    self.collapsed_rows.clear();
    self.push_recent(tree.product_type_id);
    self.recompute();
  }

  /// Rebuilds the use-stock intent from a saved tree: each `use_stock` type with a pinned facility becomes a
  /// [`StockSelection`] at that site. The drawn quantity is NOT stored — `needed` is recomputed live from the
  /// current build tree (mirroring [`toggle_stock_selection`]), so [`allocate_stock`] re-derives the draws
  /// against today's on-hand once it loads. Settings and `built` must already be restored.
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

  /// Default name for the next saved plan: the product name plus its run count.
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

  pub fn set_facility_defaults(&mut self, defaults: FacilityDefaults) {
    self.facility_defaults = defaults;
  }

  /// Replaces the on-hand stock map (keyed by `(site, type_id)`) feeding the stock-allocation pass, loaded
  /// from `store::repo::assets::on_hand_at_build_sites` for the plan's build sites.
  pub fn set_on_hand(&mut self, on_hand: HashMap<(i64, i64), i64>) {
    self.on_hand = on_hand;
    self.recompute();
  }

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.detail_pane.set_host_width(host_width);
  }

  /// Returns the facility chosen for `type_id`, falling back to the cheapest available default when the
  /// stored system id is absent from the current data (e.g. after a reload).
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

  /// The on-hand stock allocated to the current use-stock selections, draining each shared `(site, type_id)`
  /// pool in selection order. Drives the netted [`BuildPlan::raw_totals_after_stock`] the buy list surfaces;
  /// empty (no draws) until the user opts a job into stock, leaving raw totals unchanged.
  pub fn stock_allocation(&self) -> StockAllocation {
    self.derived.allocation.clone()
  }

  /// Whether `(site, type_id)` is currently opted into on-hand stock.
  pub fn is_stock_selected(&self, site: i64, type_id: i64) -> bool {
    self
      .stock_selections
      .iter()
      .any(|selection| selection.site == site && selection.type_id == type_id)
  }

  /// On-hand quantity sitting in `site`'s hangar for `type_id`, before any reservation.
  pub fn on_hand_at(&self, site: i64, type_id: i64) -> i64 {
    self.on_hand.get(&(site, type_id)).copied().unwrap_or(0)
  }

  /// Stock left in `site`'s `type_id` pool after the current selections drew against it: the on-hand total
  /// minus what the allocation already reserved. Drives "Use Stock" button visibility — a depleted pool
  /// (selected here, or consumed by an earlier toggle on the same shared pool) hides the button.
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
    // Emit one row per configured type plus the root product, every built type (so a built type with only
    // default settings is still persisted as built), and every use-stock type (a raw material drawn from
    // stock is neither configured nor built, yet its intent must persist). Deterministic order via a BTreeSet.
    let ids: BTreeSet<i64> = std::iter::once(product)
      .chain(self.settings.keys().copied())
      .chain(self.built.iter().copied())
      .chain(self.stock_selections.iter().map(|selection| selection.type_id))
      .collect();
    let types = ids
      .into_iter()
      .map(|type_id| {
        let settings = self.settings_for(type_id);
        // A use-stock type's draw site is the consuming facility, which for a raw material is not its own
        // configured facility (it has none). Persist that site in `facility_structure` so `restore` can
        // rehydrate the selection from it; the configured site stands for every other type.
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
    // Mark the memoized plan stale before dispatch when this message changes a plan input, then recompute
    // it exactly once at the end (the app-layer dirty-flag coalescing discipline). `PlanRestored` is excluded
    // because `restore` recomputes itself (it needs a fresh plan mid-restore to rehydrate stock intent).
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
      Message::PaneDrag(x) => {
        self.detail_pane.drag_to(x);
      }
      Message::PaneDragEnd => self.detail_pane.end(),
      Message::PaneDragStart => self.detail_pane.start(),
      Message::PickerScrolled {
        absolute,
      } => self.picker_scroll_offset = absolute,
      Message::PickerToggled => self.picker_open = !self.picker_open,
      // The DB round trips for save/load/delete are performed by the parent industry::update, which
      // owns the database handle; here only the resolved list and restored tree touch planner state.
      Message::PlanDeleteRequested(_) | Message::PlanLoadRequested(_) | Message::PlanSaveRequested => {}
      Message::PlanRestored(tree) => {
        self.restore(&tree);
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
      // Clipboard write is handled by the parent industry::update; nothing to do here.
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

  /// Facility-picker message arms split out of [`update`] to keep its cyclomatic complexity in check.
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

  /// Applies a live ESI facility search edit: records the query and bumps the per-keystroke generation
  /// (opening the picker for the type if typing into its always-visible field). A query that reaches
  /// [`FACILITY_SEARCH_MIN_CHARS`] marks the picker as searching; a shorter one falls back to the local
  /// list and clears any stale live results.
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
      // Typing into the always-visible field opens the picker for that type.
      None => {
        self.facility_picker = Some(FacilityPickerState {
          anchor: self.cursor.unwrap_or_default(),
          query,
          results: Vec::new(),
          search_generation: 1,
          searching: live,
          type_id,
        })
      }
    }
  }

  /// Applies live ESI facility results to the open picker, dropping them when the picker has moved to a
  /// different type or a newer keystroke has superseded this query's generation.
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

  /// ME/TE slider message arms split out of [`update`] to keep its cyclomatic complexity in check.
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

  /// Material context-menu / break-down message arms split out of [`update`] to keep its cyclomatic
  /// complexity in check.
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

  /// Derives the [`BuildNode`] computation tree for `type_id` from the live per-type settings and built set.
  fn assemble(&self, type_id: i64, seen: &mut BTreeSet<i64>) -> Option<BuildNode> {
    self.assemble_from(type_id, &self.settings, &self.built, seen)
  }

  /// Derives the [`BuildNode`] computation tree for `type_id` by walking its recipe and recursing into a
  /// material only when its type is in `built`. ME/TE/facility for each node are pulled from `settings`
  /// (fresh defaults for an unconfigured type), so the same per-type settings back every occurrence of a
  /// type. `seen` guards against a recipe cycle building itself forever.
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

  /// Marks `mat` as built in-house (no-op for a raw material with no recipe), seeding its default settings
  /// if it has not been configured yet so its per-type ME/TE/facility are addressable.
  fn break_down(&mut self, mat: i64) {
    if self.data.recipe(mat).is_none() {
      return;
    }
    let fresh = self.fresh_settings(mat);
    self.settings.entry(mat).or_insert(fresh);
    self.built.insert(mat);
  }

  /// Recursively marks every buildable input reachable from the product as built in-house, down to raw
  /// materials, in one action (manufacturing + reactions). Already-built types are kept.
  fn break_down_all(&mut self) {
    let Some(product) = self.product else {
      return;
    };
    let mut seen = BTreeSet::new();
    self.break_down_descendants(product, &mut seen);
  }

  /// Depth-first marks every buildable input of `type_id` as built; `seen` guards against recipe cycles.
  fn break_down_descendants(&mut self, type_id: i64, seen: &mut BTreeSet<i64>) {
    if !seen.insert(type_id) {
      return;
    }
    for mat in buildable_inputs(&self.data, type_id) {
      self.break_down(mat);
      self.break_down_descendants(mat, seen);
    }
  }

  /// Resolves a facility cost index for a node pinned (or not) to `facility_system`, falling back to the
  /// cheapest available default when the system is absent from current data.
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

  /// Applies a raw runs field edit: keeps only digits in the visible field, and reflows the plan from
  /// the parsed value clamped to the valid range (an empty or unparseable field holds at one run).
  fn edit_runs(&mut self, raw: String) {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    self.runs = digits.parse::<i64>().unwrap_or(1).clamp(1, RUNS_MAX);
    self.runs_input = digits;
  }

  fn fresh_settings(&self, type_id: i64) -> TypeSettings {
    fresh_settings(&self.data, &self.facility_defaults, type_id)
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

  /// Total acquisition cost of a build plan: every raw input priced at market plus the install fee of
  /// each in-house sub-build. `cost_index` resolves a type's facility cost index (0.0 when none). Jobs are
  /// deduped by type via [`BuildPlan::merged_build_order`], so each built type is charged its install fee
  /// once for its summed run count.
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
        let produced = job.node.output_per_run * job.runs;
        self.data.price(job.type_id) * produced as f64 * cost_index(job.type_id) * INSTALL_FEE_RATE
      })
      .sum();

    acquisition + sub_fees
  }

  /// Mutable per-type settings for `type_id`, inserting fresh defaults the first time the type is edited.
  fn settings_mut(&mut self, type_id: i64) -> &mut TypeSettings {
    let fresh = self.fresh_settings(type_id);
    self.settings.entry(type_id).or_insert(fresh)
  }

  fn push_recent(&mut self, type_id: i64) {
    self.recent.retain(|&id| id != type_id);
    self.recent.insert(0, type_id);
    self.recent.truncate(RECENT_LIMIT);
  }

  /// Rebuilds the memoized [`Derived`] plan from the current inputs (product, runs, settings, built, stock
  /// selections, on-hand). Called whenever any of those change — every plan-affecting `update` arm, plus
  /// `apply_data`/`set_on_hand`/`restore` — so `view` reads a ready plan instead of re-walking the tree per
  /// frame. Clears the `dirty` flag.
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

  /// Populates the initial recent list from owned blueprints in catalog order;
  /// falls back to the first catalog entries when no blueprints are owned.
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
    // A fresh product resets every per-type decision; its own default settings seed lazily via `settings_for`.
    self.settings.clear();
    self.built.clear();
    self.stock_selections.clear();
    self.collapsed_rows.clear();
    self.facility_picker = None;
  }

  fn set_runs(&mut self, runs: i64) {
    self.runs = runs.clamp(1, RUNS_MAX);
    self.runs_input = self.runs.to_string();
  }

  /// Adds or removes the `(site, type_id)` use-stock selection. Toggling off drops every selection naming the
  /// pool; toggling on appends one demanding the type's whole raw need (the draw is capped at the pool in
  /// [`allocate_stock`], so over-demanding is harmless). Append order is load-bearing for shared-pool draining.
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

  /// The whole raw (to-acquire) demand of `type_id` across the current plan, before stock netting. Used as a
  /// selection's `needed` so a use-stock toggle reaches for as much of the pool as the plan can absorb. Reads
  /// the memoized pre-netting raw totals; stock selections do not change them, so the cache is always current
  /// for a toggle (and [`restore`] refreshes the plan before rehydrating selections).
  fn raw_demand_for(&self, type_id: i64) -> i64 {
    self
      .derived
      .raw_totals
      .iter()
      .find(|total| total.type_id == type_id)
      .map(|total| total.qty)
      .unwrap_or(0)
  }

  /// Collapses or expands `type_id`'s nested material-plan subtree. Collapsing only hides the rows it builds
  /// from; the type stays built and still rolls up into the bill of materials and build order.
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
        anchor: self.cursor.unwrap_or_default(),
        query: String::new(),
        results: Vec::new(),
        search_generation: 0,
        searching: false,
        type_id,
      });
    }
  }

  /// Whole-plan economics for a saved plan, recomputed at current prices. Mirrors [`Planner::economics`]
  /// but reads the product, runs, and per-type ME/TE/facility from `tree` instead of live state, so a list
  /// of saved plans reflects today's market without rehydrating each into the live planner.
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
    let install_fee = revenue * cost_index * INSTALL_FEE_RATE;
    let profit = revenue - material_cost - install_fee;
    let margin = if revenue > 0.0 { profit / revenue * 100.0 } else { 0.0 };
    let per_unit = if output_qty > 0 {
      profit / output_qty as f64
    } else {
      0.0
    };
    let build_time_secs = node_build_time(recipe, runs, te);

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

/// Fresh, default settings for `type_id`: reactions zero out ME/TE, manufacturing inherits an owned
/// blueprint's ME/TE or the planner defaults, and the facility seeds from the configured default install
/// structure for the type's own activity (manufacturing vs reaction). Used for any type the user has not yet
/// configured — root product or sub-build alike.
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

/// Resolves the configured default install structure for an activity to `(structure id, solar system id)`,
/// when that facility is present in current data. Returns `(None, None)` — preserving the cheapest-index
/// fallback — when no default is configured or the configured facility is gone (e.g. a pinned structure that
/// is no longer accessible), so the structure id and system stay consistent.
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

/// The buildable inputs of `type_id` (materials that have their own recipe), in recipe order. A material
/// without a recipe is raw and cannot be built.
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

/// Returns total build time in seconds. `te` is a 0–20 integer (EVE TE %, applied as
/// `te / 100`). Reactions ignore TE; the raw `time_per_run × runs` is returned unchanged.
pub fn node_build_time(recipe: &Recipe, runs: i64, te: i64) -> f64 {
  let base = recipe.time_per_run as f64 * runs as f64;
  if recipe.is_reaction {
    base
  } else {
    base * (1.0 - te as f64 / 100.0)
  }
}

pub fn load(db: crate::store::Database, scope: Scope) -> iced::Task<PlannerData> {
  iced::Task::perform(async move { planner_loaders::load_data(&db, scope).await }, |data| data)
}

pub fn view<'a>(planner: &'a Planner, _scope: Scope) -> iced::Element<'a, Message> {
  use iced::{
    Length,
    widget::{Space, Stack, mouse_area},
  };

  use crate::ui::components::{backdrop, context_menu};

  if !planner.is_loaded() {
    return view::loading();
  }

  let base = mouse_area(view::body(planner)).on_move(Message::CursorMoved).into();

  // The Material Plan scrollable lives inside `base`. The root element must keep a stable widget
  // identity whether or not an overlay is open, otherwise iced rebuilds the scrollable and resets
  // its offset to the top. Always return the same Stack shape — an empty overlay slot when nothing
  // is open — so opening a menu/picker and breaking a node down move the scroll position not at all.
  let overlay: iced::Element<'a, Message> = if let Some(menu) = planner.menu() {
    let mut items = Vec::new();
    if !menu.buildable {
      items.push(context_menu::Item::disabled("Raw material \u{2014} can't break down"));
    } else if menu.built {
      items.push(context_menu::Item::action(
        "Stop building \u{2014} buy on market",
        Message::NodeCollapsed {
          type_id: menu.mat,
        },
      ));
    } else {
      items.push(context_menu::Item::warning(
        "Break down \u{2014} build in-house",
        Message::NodeBrokenDown {
          type_id: menu.mat,
        },
      ));
    }

    let title = planner.data().name(menu.mat);
    Stack::with_children(vec![
      backdrop::click_catcher(Message::MenuClosed),
      context_menu::context_menu(&title, items, menu.anchor),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
  } else if let Some(state) = planner.facility_picker() {
    Stack::with_children(vec![
      backdrop::click_catcher(Message::FacilityPickerToggled {
        type_id: state.type_id,
      }),
      view::facility_picker_panel(planner),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
  } else {
    Space::new().width(Length::Shrink).height(Length::Shrink).into()
  };

  Stack::with_children(vec![base, overlay])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

mod view {
  use iced::{
    Background, Border, ContentFit, Element, Length, Padding,
    alignment::{Horizontal, Vertical},
    widget::{Column, Row, Space, button, container, image, mouse_area, scrollable, slider, text, text_input},
  };

  use super::{Economics, MATERIAL_PLAN_SCROLL_ID, Message, Planner, RightTab, SavedPlan, node_build_time};
  use crate::{
    features::industry::{
      planner_loaders::{Category, OwnedSummary, PlannerData, PlannerFacility, Recipe},
      planner_model::{MergedBuildJob, NeededBlueprint, eff_qty, needed_blueprints_from, runs_for},
    },
    store::images::IconResolution,
    ui::{
      components::{
        badge::badge,
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

  const ESTIMATED_PICKER_ROW: f32 = 52.0;
  const FACILITY_PICKER_GAP: f32 = 6.0;
  const FACILITY_PICKER_WIDTH: f32 = 450.0;
  /// Smallest id EVE assigns a player-owned structure; NPC stations sit well below it. A live result at or
  /// above this id is a structure that must be pinned (persisted) when selected, since it never reaches the
  /// SDE/corp-sync facility tables.
  const MIN_STRUCTURE_ID: i64 = 1_000_000_000_000;
  const PANE_PADDING: f32 = 24.0;
  const PICKER_MAX_RESULTS: usize = 200;
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
    // The right pane (Detail / Plans) stays mounted whether or not a product is chosen, so saved plans
    // and the cost summary are always reachable. Only the left column swaps to the empty search state.
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
      text("Loading build catalog\u{2026}")
        .font(typography::body::REGULAR)
        .size(typography::size::LG)
        .style(typography::colored(color::text::tertiary())),
    )
  }

  fn left_pane<'a>(planner: &'a Planner, product: i64, recipe: &'a Recipe) -> Element<'a, Message> {
    let has_plan = planner.has_plan();
    // One distinct card per built type: the deduped merged build order is the source of truth, so a type
    // built by several jobs shows a single summed card. Card count (built types + the root product) reads
    // as the step count.
    let merged = planner.merged_build_order();
    let steps = merged.len().max(1);

    let mut children: Vec<Element<'a, Message>> = vec![
      picker(planner),
      section_label("Blueprints", (steps > 1).then(|| format!("{steps} steps"))),
      blueprint_card(planner, product, recipe, None),
    ];

    // Sub-builds: every non-root merged row, summed across its occurrences, rendered as a flat (un-indented)
    // card. `merged_build_order` is producer-before-consumer, so reverse it to read top-down from the cards
    // nearest the product.
    for job in merged.iter().filter(|job| !job.is_root).rev() {
      children.push(sub_blueprint_card(planner, job));
    }

    let me_hint = if recipe.is_reaction {
      "reaction inputs".to_owned()
    } else {
      format!("ME {} applied", planner.settings_for(product).me)
    };
    children.push(material_plan_header(
      planner,
      format!("{me_hint} \u{00B7} break down an item or right-click for options"),
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
      .align_y(Vertical::Center)
      .into();

    // Keep the search input at a STABLE tree position (always child 0 of this Column): returning a bare
    // `bar` when closed and a `Column[bar, results]` once the user types reparents the text input, which
    // makes iced drop its focus after the first keystroke. Always render the Column; the results slot
    // collapses to a zero-height Space (and the gap to 0) when the picker is closed.
    let open = planner.picker_open() || !planner.search().is_empty();
    let results: Element<'_, Message> = if open {
      picker_results(planner)
    } else {
      Space::new().into()
    };

    Column::with_children(vec![bar, results])
      .spacing(if open { spacing::SPACE_2 } else { 0.0 })
      .width(Length::Fill)
      .into()
  }

  fn picker_results(planner: &Planner) -> Element<'_, Message> {
    let data = planner.data();
    let query = planner.search().trim().to_lowercase();
    let category = planner.category();

    let chips: Vec<Element<'_, Message>> = std::iter::once(category_chip("All", Category::Other, true, category))
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

    let header = if query.is_empty() && matches.is_empty() {
      "Your blueprints / recent".to_owned()
    } else {
      format!("{} result{}", matches.len(), if matches.len() == 1 { "" } else { "s" })
    };

    let list: Element<'_, Message> = if matches.is_empty() {
      let source = if query.is_empty() { planner.recent() } else { &[] };
      if source.is_empty() {
        let message = if query.is_empty() {
          "No products match.".to_owned()
        } else {
          format!("No products match \u{201C}{}\u{201D}.", planner.search().trim())
        };
        centered(
          text(message)
            .font(typography::body::REGULAR)
            .size(typography::size::MD)
            .style(typography::colored(color::text::tertiary())),
        )
      } else {
        let rows: Vec<Element<'_, Message>> = source.iter().map(|&id| picker_row(planner, id)).collect();
        Column::with_children(rows).width(Length::Fill).into()
      }
    } else {
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
    };

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

  fn picker_row(planner: &Planner, type_id: i64) -> Element<'_, Message> {
    let data = planner.data();
    let recipe = data.recipe(type_id);
    let is_reaction = recipe.is_some_and(|recipe| recipe.is_reaction);

    let title = text(data.name(type_id))
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY));

    let subtitle = text(format!("{} ISK", fmt_isk(data.price(type_id))))
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

  fn category_chip(label: &str, category: Category, is_all: bool, active: Category) -> Element<'_, Message> {
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
      text(label.to_owned())
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

  /// One flat editable card for a built item type. The root product (`job = None`) carries the editable runs
  /// stepper and no relationship subline; a sub-build (`job = Some`) is keyed by its summed merged-order row,
  /// so its runs are locked to summed parent demand, its ME/TE/facility apply to every occurrence, and it
  /// shows the build-vs-buy readout.
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

    // Runs sit on the left, the ME/TE sliders are centered, and the location search is floated right.
    let mut center: Vec<Element<'a, Message>> = Vec::new();
    if !is_reaction {
      center.push(efficiency_slider(
        "Material efficiency",
        config.me,
        super::ME_MAX,
        type_id,
        true,
      ));
      center.push(efficiency_slider(
        "Time efficiency",
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

  /// A flat (un-indented) sub-build card for one merged-order row.
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
      badges.push(badge("BUILDING", Some(color::status::WARNING)))
    } else {
      badges.push(owned_badge(planner, type_id))
    };

    let subtitle = match job {
      Some(job) => format!(
        "builds {} \u{00B7} needs {} \u{00B7} {}",
        fmt_num(data.recipe(type_id).map(|r| r.output_per_run).unwrap_or(1) * job.runs),
        fmt_num(job.needed_qty),
        merged_feeds_line(data, job)
      ),
      None => format!("{} ISK each", fmt_isk(data.price(type_id))),
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
    let label = format!(
      "{}{}",
      if is_reaction { "Cycles" } else { "Runs" },
      if locked { " \u{00B7} locked" } else { "" }
    );

    let value: Element<'a, Message> = if locked {
      Row::with_children(vec![
        text(fmt_num(runs))
          .font(typography::mono::MEDIUM)
          .size(typography::size::LG)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
        text("FOR JOB")
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

  /// The runs control as one cohesive segmented box: a left `−` step, a narrow centered editable field, and
  /// a right `+` step share a single rounded border and sunken background, divided only by hairline rules
  /// (mirroring the design's `NumberStepper`). The field is capped to roughly three digits.
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
    let prefix = if material { "ME" } else { "TE" };
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
        text(format!("{prefix} {value}"))
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

    Column::with_children(vec![micro_label("Build at"), trigger])
      .spacing(spacing::SPACE_2)
      .width(Length::Fixed(FACILITY_PICKER_WIDTH))
      .into()
  }

  /// Floating results for the open "Build at" picker. Rendered in the planner's overlay Stack and anchored
  /// directly under the always-visible facility input (right-aligned and width-matched to it) so it reads as
  /// the input's popover. Keeping it in the overlay rather than inline means the card never resizes when the
  /// picker opens.
  pub(super) fn facility_picker_panel(planner: &Planner) -> Element<'_, Message> {
    let Some(state) = planner.facility_picker() else {
      return Space::new().into();
    };
    let anchor_top = (state.anchor.y + FACILITY_PICKER_GAP).max(0.0);
    let type_id = state.type_id;
    let is_reaction = planner
      .data()
      .recipe(type_id)
      .map(|recipe| recipe.is_reaction)
      .unwrap_or(false);
    let selected = planner
      .selected_facility(type_id, is_reaction)
      .map(|f| facility_ref(f, is_reaction));

    // Type-to-search only (live ESI over any reachable station/structure), identical to the Settings
    // Industry picker. The full accessible-facility set is thousands of NPC stations, so rendering it
    // unprompted made the picker lag — the trigger already shows the current selection, and a query
    // surfaces the rest on demand.
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
      .width(Length::Fixed(FACILITY_PICKER_WIDTH))
      .searching(state.searching)
      .selection(selected)
      .popover();

    let panel = container(popover).style(|_| container::Style {
      shadow: crate::ui::style::shadow::CARD,
      ..container::Style::default()
    });

    // Right padding clears the detail pane and the card/pane gutters so the panel's right edge lines up with
    // the right-floated facility input rather than the planner's right edge.
    let right = planner.detail_pane_width() + PANE_PADDING + spacing::SPACE_3_5;
    crate::ui::components::positioned_dropdown::positioned_dropdown_right(panel.into(), anchor_top, right)
  }

  /// The pin descriptor for a selected facility: `Some` for a player structure that must be persisted,
  /// `None` for an NPC station already known to the SDE.
  fn pin_for(facility: &FacilityRef) -> Option<super::PinnedStructure> {
    (facility.id >= MIN_STRUCTURE_ID).then(|| super::PinnedStructure {
      id: facility.id,
      name: facility.name.clone(),
      solar_system_id: facility.solar_system_id,
      type_id: facility.type_id,
    })
  }

  /// Projects a [`PlannerFacility`] into the shared [`FacilityRef`], carrying the cost index for the active
  /// activity (manufacturing or reaction) so the combobox can surface it per-row.
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
    let material_cost: f64 = recipe
      .materials
      .iter()
      .map(|m| eff_qty(m.base_qty, job.runs, config.me, recipe.is_reaction) as f64 * data.price(m.type_id))
      .sum();
    let produced = recipe.output_per_run * job.runs;
    let fee =
      data.price(type_id) * produced as f64 * planner.cost_index(type_id).unwrap_or(0.0) * super::INSTALL_FEE_RATE;
    let build_cost = material_cost + fee;
    let buy_cost = job.needed_qty as f64 * data.price(type_id);
    let savings = buy_cost - build_cost;
    let build_time = node_build_time(recipe, job.runs, config.te);

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

  /// The Material Plan section heading: a clickable collapse/expand toggle (the table is collapsed on initial
  /// open) on the left and, when the plan has at least one buildable input, a warning-tinted "Break down all"
  /// button floated right that recursively builds every buildable input down to raw materials in one action.
  fn material_plan_header(planner: &Planner, hint: String) -> Element<'_, Message> {
    let label = section_label("Material plan", Some(hint));
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
      text("Break down all")
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

    acc.out.push(footer_row("Material cost", &fmt_isk_full(acc.total)));

    container(Column::with_children(acc.out).width(Length::Fill))
      .width(Length::Fill)
      .style(bordered_table)
      .into()
  }

  /// The mutable accumulators threaded through the recursive [`material_rows`] walk: the emitted rows, the
  /// cycle guard, and the running depth-0 material cost.
  struct MaterialRowsAcc<'a> {
    out: Vec<Element<'a, Message>>,
    seen: std::collections::BTreeSet<i64>,
    total: f64,
  }

  /// Recursively emits the material-plan rows for `recipe`, descending into a material when its type is built
  /// in-house (keyed per-TYPE, not per tree position). The accumulator's `seen` set guards against a recipe
  /// cycle recursing forever.
  fn material_rows<'a>(
    planner: &'a Planner,
    recipe: &'a Recipe,
    runs: i64,
    site: Option<i64>,
    depth: usize,
    acc: &mut MaterialRowsAcc<'a>,
  ) {
    let data = planner.data();
    for material in &recipe.materials {
      let qty = eff_qty(material.base_qty, runs, runs_me(planner, recipe), recipe.is_reaction);
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

  /// The ME the planner applies for the type that owns `recipe` (reactions ignore ME). Per-TYPE, so the same
  /// value backs the material plan and the editable cards.
  fn runs_me(planner: &Planner, recipe: &Recipe) -> i64 {
    if recipe.is_reaction {
      return 0;
    }
    planner
      .product_for_blueprint(recipe.blueprint_type_id)
      .map(|product| planner.settings_for(product).me)
      .unwrap_or(super::DEFAULT_ME)
  }

  /// Inline "Breakdown" button shown on a buildable material-plan row that has not yet been broken down.
  /// Fires the same `NodeBrokenDown` message the row-click and context menu use, so all three coexist.
  fn breakdown_button<'a>(type_id: i64) -> Element<'a, Message> {
    let inner = Row::with_children(vec![
      Icon::flask()
        .color(color::status::WARNING)
        .size(11.0)
        .render::<Message>(),
      text("Breakdown")
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

  /// "Use Stock" toggle shown on a material-plan row when its consuming build site still holds uncommitted
  /// on-hand stock of the type. Fires [`Message::StockSelectionToggled`] keyed by `(site, type)`.
  fn use_stock_button<'a>(site: i64, type_id: i64) -> Element<'a, Message> {
    let inner = Row::with_children(vec![
      Icon::assets()
        .color(color::status::ONLINE)
        .size(11.0)
        .render::<Message>(),
      text("Use Stock")
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

  /// The active "STOCK" chip a use-stock row shows in place of the toggle; clicking it stops drawing stock.
  fn stock_chip<'a>(site: i64, type_id: i64) -> Element<'a, Message> {
    let inner = Row::with_children(vec![
      Icon::check().color(color::status::ONLINE).size(9.0).render::<Message>(),
      text("STOCK")
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

  /// The quantity cell with a from-stock / to-buy split subline. When stock covers part of the line, the total
  /// sits above a `{drawn} stock \u{00B7} {remaining} {build|buy}` breakdown; an uncovered line shows only the total.
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
        text(format!("{} stock", fmt_num(drawn)))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::status::ONLINE))
          .into(),
      ];
      if remaining > 0 {
        let (label, tint) = if building {
          (format!("{} build", fmt_num(remaining)), color::status::WARNING)
        } else {
          (format!("{} buy", fmt_num(remaining)), color::text::tertiary())
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

  /// How a material-plan line is sourced from on-hand stock: whether it is opted in, how much the toggle draws
  /// from the site pool (capped at the line's demand), what is left, and whether an unselected pool still has
  /// uncommitted stock (so the "Use Stock" button should show).
  struct StockSplit {
    can_use: bool,
    drawn: i64,
    remaining: i64,
    site: Option<i64>,
    using: bool,
  }

  /// Resolves the [`StockSplit`] for `type_id` at its consuming build `site`: a selected line draws
  /// `min(on_hand, qty)`; an unselected one can use stock when the pool still has an uncommitted remainder
  /// (its own, or a shared pool an earlier toggle has not drained).
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

  /// The stock affordance for a material-plan row: the active "STOCK" chip when opted in, the "Use Stock"
  /// button when the site pool still has uncommitted stock, or nothing.
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

  /// The per-row expand/collapse affordance shown on a built material-plan row: a rotating chevron that
  /// hides or reveals just that row's nested subtree. Right-pointing when collapsed, down when expanded.
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
      name_row = name_row.push(badge("BUILDING", Some(color::status::WARNING)));
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

    // To-buy cost subtracts the drawn-from-stock units from each type's buy quantity; the inventory value is
    // those same drawn units priced at market. Buy + inventory equals the un-netted acquisition total.
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
      rows.push(footer_row("Covered from inventory", &fmt_isk_full(inventory_value)));
    }
    let footer_label = if inventory_value > 0.0 {
      "Cost to buy"
    } else {
      "Acquisition cost"
    };
    rows.push(footer_row(footer_label, &fmt_isk_full(buy_cost)));

    let hint = if stocked > 0 {
      format!(
        "raw inputs to acquire \u{00B7} {} items \u{00B7} {stocked} drawn from stock",
        totals.len()
      )
    } else {
      format!("raw inputs to acquire \u{00B7} {} items", totals.len())
    };

    Column::with_children(vec![
      section_label("Bill of materials", Some(hint)),
      container(Column::with_children(rows).width(Length::Fill))
        .width(Length::Fill)
        .style(bordered_table)
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .into()
  }

  /// Stock drawn for a bill-of-materials line, capped at its demand (never more than the line needs).
  fn drawn_qty(allocation: &super::StockAllocation, total: &super::RawTotal) -> i64 {
    allocation.drawn_for_type(total.type_id).min(total.qty).max(0)
  }

  /// The to-buy remainder of a bill-of-materials line: its raw demand minus the stock drawn for the type.
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
        head("MATERIAL", None),
        head("TOTAL", Some(COL_BOM_QTY)),
        head("FROM STOCK", Some(COL_BOM_QTY)),
        head("TO BUY", Some(COL_BOM_QTY)),
        head("SUBTOTAL", Some(COL_COST)),
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

    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for (index, job) in jobs.iter().enumerate() {
      rows.push(build_order_row(planner, index, job));
    }

    Column::with_children(vec![
      section_label(
        "Build order",
        Some(format!(
          "{count} job{} \u{00B7} dependencies first",
          if count == 1 { "" } else { "s" }
        )),
      ),
      container(Column::with_children(rows).width(Length::Fill))
        .width(Length::Fill)
        .style(bordered_table)
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .into()
  }

  /// One build-order row: numbered index, item tile, name + activity badge with a `feeds →` / `final product`
  /// subline, a prominent `×N` runs/cycles pill, and the build time. The final-product row is plasma-accented.
  fn build_order_row<'a>(planner: &'a Planner, index: usize, job: &MergedBuildJob) -> Element<'a, Message> {
    let data = planner.data();
    let is_final = job.is_root;
    let time = node_build_time(&recipe_for(data, job.type_id), job.runs, job.node.te);

    let body = Row::with_children(vec![
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
        Row::with_children(vec![
          text(data.name(job.type_id))
            .font(typography::body::MEDIUM)
            .size(typography::size::MD)
            .style(typography::colored(color::text::PRIMARY))
            .into(),
          activity_badge(job.node.is_reaction),
        ])
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
      runs_pill(job.runs, job.node.is_reaction, is_final),
      text(fmt_duration_coarse(time as i64))
        .font(typography::mono::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY))
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .align_y(Vertical::Center)
    .width(Length::Fill);

    container(body)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_3,
        bottom: spacing::SPACE_3,
        left: spacing::SPACE_3,
        right: spacing::SPACE_3,
      })
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

  /// The bordered `×N` runs (manufacturing) / cycles (reaction) pill for a build-order row: a large count over
  /// an uppercase RUNS/CYCLES label. The final-product pill is plasma-accented to match its row.
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
      text(if is_reaction { "CYCLES" } else { "RUNS" })
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
      return "final product".to_owned();
    }
    match job.consumers.as_slice() {
      [consumer] => format!("feeds \u{2192} {}", data.name(*consumer)),
      consumers => format!("feeds \u{2192} {} jobs", consumers.len()),
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

    let hint = format!(
      "{count} blueprint{} \u{00B7} {}",
      if count == 1 { "" } else { "s" },
      if missing > 0 {
        format!("{missing} to acquire")
      } else {
        "all owned".to_owned()
      }
    );

    Column::with_children(vec![
      section_label("Needed blueprints", Some(hint)),
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
    let kind_word = if recipe.is_reaction { "Formula" } else { "Blueprint" };
    let unit = if recipe.is_reaction { "cycles" } else { "runs" };

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
        text(format!(
          "{} job{} \u{00B7} \u{00D7}{} {unit} total",
          blueprint.jobs,
          if blueprint.jobs == 1 { "" } else { "s" },
          fmt_num(blueprint.runs)
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

  /// The acquire-status pill: owned reuses the BPO/BPC + ME badge wording (in-scope vs held-elsewhere), and a
  /// missing blueprint shows the amber "BUY / INVENT" pill.
  fn blueprint_status_pill<'a>(owned: Option<&OwnedSummary>) -> Element<'a, Message> {
    match owned {
      Some(summary) => {
        let mut label = if summary.is_original {
          "BPO".to_owned()
        } else {
          "BPC".to_owned()
        };
        if summary.material_efficiency > 0 {
          label.push_str(&format!(" \u{00B7} ME{}", summary.material_efficiency));
        }
        if !summary.in_scope {
          label.push_str(" \u{00B7} ELSEWHERE");
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
      None => badge("BUY / INVENT", Some(color::status::WARNING)),
    }
  }

  /// Resolves the blueprint (BPO/BPC) icon keyed on the recipe's `blueprint_type_id` — mirrors the Blueprints
  /// tab tile. `is_copy` selects the BPC variant; `None` falls back to the BPO variant (unowned defaults to
  /// BPO). Missing icons fall back to the copy glyph.
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
          text("Search a product to see its cost, profit, and shopping list.")
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
        text(format!(
          "\u{00D7}{} produced \u{00B7} {} runs",
          fmt_num(eco.output_qty),
          planner.runs()
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
        micro_label("Estimated profit"),
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
          text(format!("{} margin", fmt_pct(eco.margin)))
            .font(typography::mono::MEDIUM)
            .size(typography::size::MD)
            .style(typography::colored(profit_color))
            .into(),
          text(format!("{}/unit", fmt_isk(eco.per_unit)))
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
      detail_line("Revenue", &fmt_isk_full(eco.revenue), color::text::PRIMARY, false),
      detail_line(
        "Material cost",
        &format!("\u{2212}{}", fmt_isk_full(eco.material_cost)),
        color::status::DANGER,
        false,
      ),
      detail_line(
        "Job fee",
        &format!("\u{2212}{}", fmt_isk_full(eco.install_fee)),
        color::status::DANGER,
        false,
      ),
      detail_line(
        "Net profit",
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
        "Build time",
        &fmt_duration_coarse(eco.build_time_secs as i64),
      ),
      meta_line(Icon::wallet(), "ISK / hour", &fmt_isk(eco.isk_per_hour())),
      meta_line(Icon::assets(), "Output volume", &fmt_volume(eco.output_volume)),
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
          "Blueprint original".to_owned()
        } else {
          "Blueprint copy".to_owned()
        },
        format!(
          "{} \u{00B7} {} \u{00B7} ME {} \u{00B7} TE {}",
          if summary.is_original { "BPO" } else { "BPC" },
          if summary.in_scope { "in scope" } else { "held elsewhere" },
          summary.material_efficiency,
          summary.time_efficiency
        ),
      ),
      None => (
        Icon::help(),
        color::status::WARNING,
        "No blueprint owned".to_owned(),
        "planning only \u{2014} buy or invent one to build".to_owned(),
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
    let enabled = planner.product().is_some();
    let mut control = button(
      Row::with_children(vec![
        Icon::doc().color(color::accent::PLASMA).size(14.0).render::<Message>(),
        text("Save build plan")
          .font(typography::body::MEDIUM)
          .size(typography::size::MD)
          .style(typography::colored(color::accent::PLASMA))
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_3)
    .style(move |_, status| {
      let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
      button::Style {
        background: Some(Background::Color(color::with_alpha(
          color::accent::PLASMA,
          if hovered { 0.22 } else { 0.14 },
        ))),
        border: Border {
          color: color::accent::PLASMA,
          radius: radius::CARD.into(),
          width: 1.0,
        },
        text_color: color::accent::PLASMA,
        ..button::Style::default()
      }
    });
    if enabled {
      control = control.on_press(Message::PlanSaveRequested);
    }
    control.into()
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
      plan_action("Load", color::accent::PLASMA, Message::PlanLoadRequested(plan.id)),
      plan_action("Delete", color::status::DANGER, Message::PlanDeleteRequested(plan.id)),
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
      return text("Recipe unavailable at current data")
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
        "Profit",
        &format!(
          "{}{}",
          if eco.profit >= 0.0 { "+" } else { "\u{2212}" },
          fmt_isk(eco.profit.abs())
        ),
        profit_color,
      ),
      metric("Margin", &fmt_pct(eco.margin), profit_color),
      metric("Revenue", &fmt_isk(eco.revenue), color::text::secondary()),
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
        text("No saved plans yet")
          .font(typography::body::REGULAR)
          .size(typography::size::LG)
          .style(typography::colored(color::text::secondary()))
          .into(),
        text("Configure a build, then save it here.")
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
    button(
      Row::with_children(vec![
        Icon::copy().color(color::text::PRIMARY).size(14.0).render::<Message>(),
        text("Copy shopping list")
          .font(typography::body::MEDIUM)
          .size(typography::size::MD)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
      ])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding(spacing::SPACE_3)
    .on_press(Message::ShoppingListCopied)
    .style(crate::ui::style::control::ghost_button)
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
        head("MATERIAL", None, false),
        head("QUANTITY", Some(COL_QTY), true),
        head("UNIT PRICE", Some(COL_PRICE), true),
        head("SUBTOTAL", Some(COL_COST), true),
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
        text(format!("\u{00B7} {hint}"))
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
      badge("REACTION", Some(color::status::WARNING))
    } else {
      badge("MANUFACTURING", Some(color::accent::PLASMA))
    }
  }

  fn owned_badge<'a>(planner: &Planner, type_id: i64) -> Element<'a, Message> {
    match planner.data().owned.get(&type_id) {
      Some(summary) => badge(
        if summary.is_original { "BPO" } else { "BPC" },
        Some(if summary.in_scope {
          color::status::ONLINE
        } else {
          color::text::secondary()
        }),
      ),
      None => badge("NO BP", None),
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

  /// The left column's no-product cold-open state. The product search bar stays pinned at the top (so the
  /// user can actually search for something to build); a centered hint fills the space below until the
  /// picker is opened or a query is typed, at which point the picker's own results take over. Returned
  /// bare (no scrollable) — [`body`] wraps the left column in the shared scrollable.
  fn empty_left(planner: &Planner) -> Element<'_, Message> {
    let mut children: Vec<Element<'_, Message>> = vec![picker(planner)];

    if !planner.picker_open() && planner.search().is_empty() {
      children.push(centered(
        text("Search a product to start planning a build.")
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
    fn it_ignores_a_breakdown_request_for_a_raw_material() {
      let mut planner = planner();

      planner.update(Message::NodeBrokenDown {
        type_id: TRITANIUM,
      });

      assert!(!planner.is_built(TRITANIUM));
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
      // 5 direct + 2 retrievers × 10 = 25.
      assert_eq!(tritanium.qty, 25);
      assert!(totals.iter().all(|t| t.type_id != RETRIEVER));
    }
  }

  mod break_down_all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_recursively_breaks_down_every_buildable_input_to_raw() {
      let mut planner = planner();

      planner.update(Message::BreakDownAll);

      // The buildable RETRIEVER input is built in-house; raw TRITANIUM is left to buy.
      assert!(planner.is_built(RETRIEVER));
      assert!(!planner.is_built(TRITANIUM));
      // The derived tree builds RETRIEVER but leaves its raw TRITANIUM to buy.
      let root = planner.plan().unwrap().root;
      assert!(root.children.contains_key(&RETRIEVER));
      assert!(root.children[&RETRIEVER].children.is_empty());
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
    fn it_breaks_down_a_reaction_input() {
      const FUEL: i64 = 4051;
      const COMPOSITE: i64 = 16_670;
      const GAS: i64 = 25_268;
      let mut data = PlannerData::default();
      // FUEL reaction consumes a buildable COMPOSITE reaction, which consumes raw GAS.
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
    fn it_reports_buildable_inputs_only_when_present() {
      let planner = planner();
      assert!(planner.has_buildable_inputs());

      // A product whose only inputs are raw has nothing to break down.
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

  mod seed_from_blueprint {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_seeds_the_product_a_blueprint_makes() {
      let mut planner = planner();
      planner.update(Message::ProductPicked(RETRIEVER));

      // The Hulk blueprint type is HULK + 1 (see the `planner()` recipe helper).
      let seeded = planner.seed_from_blueprint(HULK + 1);

      assert!(seeded);
      assert_eq!(planner.product(), Some(HULK));
      assert!(!planner.is_built(RETRIEVER));
    }

    #[test]
    fn it_reports_no_seed_for_an_unknown_blueprint() {
      let mut planner = planner();

      assert!(!planner.seed_from_blueprint(123_456));
    }

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

      // A reaction picks the reactions structure id, a manufacturing product picks the manufacturing one —
      // the install structure (not just its system) must differ per activity.
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

  mod row_collapse {
    use iced::advanced::widget::Tree;

    use super::*;

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

    #[test]
    fn it_keeps_a_collapsed_row_built_and_in_the_raw_totals() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      planner.update(Message::RowCollapseToggled {
        type_id: RETRIEVER,
      });

      // Collapsing only hides nested rows in the table; the type stays built and its raw inputs still roll up.
      assert!(planner.is_built(RETRIEVER));
      let totals = planner.plan().unwrap().raw_totals();
      assert!(totals.iter().all(|total| total.type_id != RETRIEVER));
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
  }

  mod economics {
    use pretty_assertions::assert_eq;

    use super::*;

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

      // With a component built in-house, material cost is the bill-of-materials acquisition total
      // (raw inputs only — the sub-built component is no longer bought) plus its sub-job install fee,
      // which diverges sharply from pricing the root recipe's immediate materials at market.
      assert!(eco.material_cost > acquisition);
      assert_eq!(eco.material_cost, 600_115.0);
      assert_eq!(acquisition, 115.0);
      assert_eq!(eco.profit, eco.revenue - eco.material_cost - eco.install_fee);
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

  mod memoization {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_refreshes_the_cached_plan_after_a_plan_affecting_update() {
      let mut planner = planner();
      let before = planner.merged_build_order().len();

      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });

      // Building RETRIEVER in-house adds a job to the merged order, and the memoized accessors must reflect
      // it without a fresh recompute in view().
      assert!(planner.merged_build_order().len() > before);
      assert_eq!(
        planner.merged_build_order(),
        planner.plan().unwrap().merged_build_order().as_slice()
      );
      assert_eq!(planner.raw_totals(), planner.plan().unwrap().raw_totals().as_slice());
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
    fn it_leaves_the_cached_plan_untouched_on_a_cursor_move() {
      let mut planner = planner();
      let before = planner.plan();

      planner.update(Message::CursorMoved(Point::new(10.0, 20.0)));

      assert_eq!(planner.plan(), before);
    }

    #[test]
    fn it_refreshes_the_cached_allocation_when_on_hand_loads() {
      let mut planner = planner();
      let site = 60_000_002;
      // Opt the root's raw Tritanium into stock; the pool is empty at toggle time, so nothing is drawn yet.
      planner.update(Message::StockSelectionToggled {
        site,
        type_id: TRITANIUM,
      });
      assert_eq!(planner.stock_allocation().drawn_for_type(TRITANIUM), 0);

      // Loading on-hand stock must re-run the allocation against the now-available pool.
      planner.set_on_hand(HashMap::from([((site, TRITANIUM), 3)]));

      assert_eq!(planner.stock_allocation().drawn_for_type(TRITANIUM), 3);
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

      // The root Stack must carry the same number of direct children open or closed so the Material
      // Plan scrollable keeps a stable widget identity and never resets its offset on right-click.
      assert!(planner.menu().is_some());
      let with_menu = super::super::view(&planner, Scope::All);
      assert_eq!(Tree::new(with_menu.as_widget()).children.len(), idle_children);
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
      // Open the picker on a query that matches the whole filler block, then scroll far past the first window.
      planner.update(Message::SearchChanged("filler".to_owned()));
      planner.update(Message::PickerScrolled {
        absolute: 4_000.0,
      });

      // The picker materializes the windowed rows for the recorded offset rather than only the first screenful.
      let rendered = super::super::view(&planner, Scope::All);
      let _ = Tree::new(rendered.as_widget());
      assert_eq!(planner.picker_scroll_offset(), 4_000.0);
    }

    #[test]
    fn it_renders_the_product_picker_across_empty_and_populated_result_states() {
      // Picker open with no query — the seeded catalog (Hulk) yields a populated result list.
      let mut populated = planner();
      populated.update(Message::PickerToggled);
      let _ = Tree::new(super::super::view(&populated, Scope::All).as_widget());

      // A query that matches nothing renders the "No products match <query>" empty state.
      let mut no_match = planner();
      no_match.update(Message::SearchChanged("zzzznomatch".to_owned()));
      let _ = Tree::new(super::super::view(&no_match, Scope::All).as_widget());

      // A fresh planner with the picker open and no recent products renders the bare empty state.
      let mut empty = Planner::new();
      empty.update(Message::PickerToggled);
      let _ = Tree::new(super::super::view(&empty, Scope::All).as_widget());

      // No catalog matches under the chosen category, but a recently picked product backfills the list.
      let mut recent = planner();
      recent.update(Message::PickerToggled);
      recent.update(Message::CategorySelected(Category::Module));
      let _ = Tree::new(super::super::view(&recent, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_facility_picker_panel_across_match_states() {
      // Open the facility picker on the root node — renders a row per eligible seeded facility.
      let mut root = planner();
      root.update(Message::CursorMoved(Point::new(120.0, 80.0)));
      root.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });
      let _ = Tree::new(super::super::view(&root, Scope::All).as_widget());

      // A query that matches no facility renders the "No facilities match." empty state.
      root.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "zzzznomatch".to_owned(),
      });
      let _ = Tree::new(super::super::view(&root, Scope::All).as_widget());

      // Opening the picker on a sub-node resolves the reaction flag from the node's own recipe.
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
    fn it_renders_the_needed_blueprints_section_across_owned_and_missing_states() {
      // No owned blueprints — every needed blueprint shows the amber BUY / INVENT pill, all rows tinted.
      let mut missing = planner();
      missing.update(Message::BreakDownAll);
      let _ = Tree::new(super::super::view(&missing, Scope::All).as_widget());

      // Mark the in-house RETRIEVER blueprint as owned in-scope; its row reuses the owned BPO/ME badge.
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
    fn it_renders_the_merged_build_order_with_a_multi_consumer_subline() {
      // Root SHIP consumes two buildable sub-assemblies (LEFT, RIGHT), each of which consumes the same
      // buildable PLATE — so the merged build order collapses PLATE into one row fed by two jobs.
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

      // PLATE merges to a single job consumed by both LEFT and RIGHT.
      let merged = planner.plan().unwrap().merged_build_order();
      let plate = merged.iter().find(|job| job.type_id == PLATE).unwrap();
      assert_eq!(plate.consumers.len(), 2);

      let _ = Tree::new(super::super::view(&planner, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_material_plan_grid_across_raw_and_buildable_rows() {
      // The always-visible material plan on the HULK root emits the grid header plus a raw Tritanium row and a
      // buildable Retriever row carrying the inline breakdown affordance.
      let planner = planner();

      let _ = Tree::new(super::super::view(&planner, Scope::All).as_widget());
    }

    #[test]
    fn it_renders_the_material_plan_with_a_nested_building_row() {
      // Breaking down the buildable Retriever recurses the material plan into its own materials, so the row
      // renders the BUILDING badge, the per-row collapse chevron, and the nested depth-tinted children.
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

      // Pin the root to a site holding Tritanium stock: the material row first shows the "Use Stock" button,
      // then the active "STOCK" chip and from-stock split once the line is opted in.
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
  }

  mod facility_picker {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_a_node_to_the_cheapest_eligible_facility() {
      let planner = planner();

      let facility = planner.selected_facility(HULK, false).unwrap();

      assert_eq!(facility.name, "Cheap Citadel");
      assert_eq!(facility.solar_system_id, 30_002_187);
    }

    #[test]
    fn it_resolves_the_exact_picked_structure_when_a_system_hosts_several() {
      // A second facility in the same system as "Pricey Station", sorted after it. Picking the second by
      // structure id must win over system-based resolution (which returns the system's first facility) — the
      // bug that made reactions and manual picks silently display the wrong same-system facility.
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
    fn it_applies_a_picked_facility_to_only_that_types_settings() {
      // Break down the buildable RETRIEVER, then pick a distinct facility on its sub-build card. The pick must
      // write to RETRIEVER's per-type settings only, leaving the root HULK card on its own default.
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

    #[test]
    fn it_anchors_the_popover_at_the_cursor_when_the_picker_opens() {
      let mut planner = planner();
      planner.update(Message::CursorMoved(Point::new(640.0, 215.0)));

      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });

      assert_eq!(
        planner.facility_picker().map(|state| state.anchor),
        Some(Point::new(640.0, 215.0))
      );
    }

    #[test]
    fn it_anchors_the_popover_when_typing_opens_the_picker() {
      let mut planner = planner();
      planner.update(Message::CursorMoved(Point::new(120.0, 88.0)));

      planner.update(Message::FacilitySearchChanged {
        type_id: HULK,
        query: "cheap".to_owned(),
      });

      assert_eq!(
        planner.facility_picker().map(|state| state.anchor),
        Some(Point::new(120.0, 88.0))
      );
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
    fn it_opens_a_nodes_picker_when_its_always_visible_field_is_typed_into() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        type_id: RETRIEVER,
      });
      planner.update(Message::FacilityPickerToggled {
        type_id: HULK,
      });

      // Typing into a different node's field switches the open picker to that node.
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
      // A second keystroke supersedes the first query's generation.
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

  mod detail_pane {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::window_state::UiState;

    const HOST: f32 = 1_200.0;

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

      // The handle sits on the left edge of a right-anchored pane: dragging left grows it.
      planner.update(Message::PaneDragStart);
      planner.update(Message::PaneDrag(800.0));
      planner.update(Message::PaneDrag(760.0));
      planner.update(Message::PaneDragEnd);
      let settled = planner.detail_pane_width();
      ui.panes.insert(DETAIL_PANE_KEY.to_owned(), planner.detail_pane_ratio());

      let mut restored = Planner::new().with_restored_panes(&ui);
      restored.set_pane_host_width(HOST);

      assert_eq!(settled, 380.0);
      assert_eq!(restored.detail_pane_width(), settled);
    }

    #[test]
    fn it_clamps_a_stored_width_below_the_minimum() {
      let mut ui = UiState::default();
      ui.panes.insert("main".to_owned(), HOST);
      ui.panes.insert(DETAIL_PANE_KEY.to_owned(), 0.01);

      let mut planner = Planner::new().with_restored_panes(&ui);
      planner.set_pane_host_width(HOST);

      assert_eq!(planner.detail_pane_width(), DETAIL_PANE_MIN_WIDTH);
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
    fn it_resets_the_scroll_offset_when_the_search_query_changes() {
      let mut planner = planner();
      planner.update(Message::PickerScrolled {
        absolute: 640.0,
      });

      planner.update(Message::SearchChanged("hulk".to_owned()));

      assert_eq!(planner.picker_scroll_offset(), 0.0);
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
  }

  mod runs_changed {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_runs_to_the_valid_range_and_recomputes_output() {
      let mut planner = planner();

      planner.update(Message::RunsChanged(0));
      assert_eq!(planner.runs(), 1);

      planner.update(Message::RunsChanged(50));
      assert_eq!(planner.economics().unwrap().output_qty, 50);
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

    #[test]
    fn it_keeps_only_digits_and_reflows_from_an_edited_runs_field() {
      let mut planner = planner();

      planner.update(Message::RunsInputChanged("4x2".to_owned()));

      assert_eq!(planner.runs(), 42);
      assert_eq!(planner.runs_input(), "42");
      assert_eq!(planner.economics().unwrap().output_qty, 42);
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
    fn it_clamps_an_edited_runs_field_to_the_maximum() {
      let mut planner = planner();

      planner.update(Message::RunsInputChanged("99999".to_owned()));

      assert_eq!(planner.runs(), RUNS_MAX);
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
      planner.update(Message::PlanRestored(Box::new(tree)));

      assert_eq!(planner.runs(), 4);
      assert_eq!(planner.right_tab(), RightTab::Detail);
      assert_eq!(planner.plan(), source.plan());
    }
  }

  mod search_placeholder {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_the_buildable_catalog_size() {
      let planner = planner();

      assert_eq!(planner.search_placeholder(), "Search 1 buildable products\u{2026}");
    }

    #[test]
    fn it_falls_back_when_no_catalog_is_loaded() {
      let planner = Planner::new();

      assert_eq!(planner.search_placeholder(), "Search buildable products\u{2026}");
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

  mod stock_selection {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    const SITE: i64 = 60_000_001;
    const SITE_SYSTEM: i64 = 30_000_142;

    /// A planner whose HULK root is pinned to `SITE`, with `on_hand` stock keyed by `(SITE, type)`.
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

    #[test]
    fn it_drains_the_pool_when_a_material_is_opted_into_stock() {
      // HULK root consumes 5 Tritanium directly; 4 on hand draws all 4, leaving the pool empty.
      let mut planner = sited_planner(HashMap::from([((SITE, TRITANIUM), 4)]));

      planner.update(Message::StockSelectionToggled {
        site: SITE,
        type_id: TRITANIUM,
      });

      assert_eq!(planner.remaining_pool(SITE, TRITANIUM), 0);
      assert_eq!(planner.stock_allocation().drawn_for_type(TRITANIUM), 4);
    }

    #[test]
    fn it_nets_drawn_stock_off_the_bill_of_materials() {
      // 5 Tritanium needed, 4 drawn from stock leaves 1 to buy.
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
    fn it_hides_the_button_for_a_later_consumer_once_the_shared_pool_is_drained() {
      // Both the HULK row and the broken-down RETRIEVER row consume Tritanium at the same site/pool. Opting
      // the first into stock drains the 6-unit pool, so the shared pool is empty for the second consumer.
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
    fn it_recomputes_drawn_stock_live_after_a_snapshot_restore() {
      // HULK root pinned to SITE consumes 5 Tritanium; 4 sit in SITE's hangar and are opted into stock. After
      // snapshot -> restore -> on-hand reload, the live allocation re-derives all 4 drawn units from the
      // current tree and assets — no frozen quantity is persisted.
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
    fn it_composes_stock_with_a_breakdown_on_the_remainder() {
      // Break RETRIEVER down so the plan rolls up to 25 Tritanium (5 direct + 2*10), draw 6 from stock,
      // leaving 19 to buy: the breakdown deepens the tree, the netting subtracts the drawn stock.
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
    fn it_returns_none_without_a_selected_product() {
      let planner = Planner::new();

      assert_eq!(planner.snapshot(), None);
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
    fn it_threads_the_picked_structure_onto_the_derived_tree() {
      let planner = configured_planner();

      let plan = planner.plan().unwrap();

      assert_eq!(plan.root.facility_structure, Some(60_000_001));
      let retriever = plan.root.children.get(&RETRIEVER).unwrap();
      assert_eq!(retriever.facility_structure, Some(60_000_002));
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
  }
}
