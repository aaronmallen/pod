use std::collections::BTreeMap;

use iced::Point;

use super::{
  Scope,
  planner_loaders::{self, Category, PlannerData, PlannerFacility, Recipe},
  planner_model::{BuildNode, BuildPlan, Material},
};
use crate::{
  store::repo::industry::{PlanNode, PlanTree},
  ui::components::resizable_pane::PaneDrag,
  window_state::UiState,
};

pub const DETAIL_PANE_KEY: &str = "industry.planner.detail";

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
  pub path: Vec<i64>,
  pub query: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialMenu {
  pub anchor: Point,
  pub buildable: bool,
  pub built: bool,
  pub mat: i64,
  pub parent: Vec<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
  CategorySelected(Category),
  CursorMoved(Point),
  FacilityPickerToggled { path: Vec<i64> },
  FacilitySearchChanged { path: Vec<i64>, query: String },
  FacilitySelected { path: Vec<i64>, solar_system_id: i64 },
  MaterialEfficiencyChanged { me: i64, path: Vec<i64> },
  MaterialRightPressed { mat: i64, parent: Vec<i64> },
  MenuClosed,
  NodeBrokenDown { mat: i64, parent: Vec<i64> },
  NodeCollapsed { mat: i64, parent: Vec<i64> },
  PaneDrag(f32),
  PaneDragEnd,
  PaneDragStart,
  PickerScrolled { absolute: f32 },
  PickerToggled,
  PlanDeleteRequested(i64),
  PlanLoadRequested(i64),
  PlanRestored(Box<PlanTree>),
  PlanSaveRequested,
  PlansListed(Vec<SavedPlanData>),
  ProductPicked(i64),
  RightTabSelected(RightTab),
  RunsChanged(i64),
  RunsInputChanged(String),
  SearchChanged(String),
  ShoppingListCopied,
  TimeEfficiencyChanged { path: Vec<i64>, te: i64 },
}

/// Per-node build configuration for the in-house production tree.
///
/// Each key in `children` is the type-id of a material the user has chosen to produce
/// in-house rather than buy. An absent key means "buy on market." The root node (held by
/// `Planner::tree`) represents the top-level product. `at`/`at_mut` address a node by
/// walking a slice of type-ids, so `path = [A, B]` reaches `root.children[A].children[B]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeConfig {
  pub children: BTreeMap<i64, NodeConfig>,
  pub facility_system: Option<i64>,
  pub me: i64,
  pub te: i64,
}

impl NodeConfig {
  fn rebuild(nodes: &[PlanNode]) -> NodeConfig {
    let mut root = NodeConfig::default();
    let mut ordered: Vec<&PlanNode> = nodes.iter().collect();
    // Shortest path first guarantees each parent node exists before its children are inserted.
    ordered.sort_by_key(|node| node.path.len());

    for node in ordered {
      let config = NodeConfig {
        children: BTreeMap::new(),
        facility_system: node.facility_system,
        me: node.me,
        te: node.te,
      };
      match node.path.split_last() {
        None => {
          root.facility_system = config.facility_system;
          root.me = config.me;
          root.te = config.te;
        }
        Some((&mat, parent)) => {
          if let Some(target) = root.at_mut(parent) {
            target.children.insert(mat, config);
          }
        }
      }
    }
    root
  }

  fn at(&self, path: &[i64]) -> Option<&NodeConfig> {
    let mut node = self;
    for step in path {
      node = node.children.get(step)?;
    }
    Some(node)
  }

  fn at_mut(&mut self, path: &[i64]) -> Option<&mut NodeConfig> {
    let mut node = self;
    for step in path {
      node = node.children.get_mut(step)?;
    }
    Some(node)
  }

  fn flatten(&self, path: &mut Vec<i64>, out: &mut Vec<PlanNode>) {
    out.push(PlanNode {
      facility_system: self.facility_system,
      me: self.me,
      path: path.clone(),
      te: self.te,
    });
    for (&mat, child) in &self.children {
      path.push(mat);
      child.flatten(path, out);
      path.pop();
    }
  }
}

#[derive(Debug)]
pub struct Planner {
  category: Category,
  cursor: Option<Point>,
  data: PlannerData,
  detail_pane: PaneDrag,
  facility_picker: Option<FacilityPickerState>,
  loaded: bool,
  menu: Option<MaterialMenu>,
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
  tree: NodeConfig,
}

impl Planner {
  pub fn new() -> Self {
    Planner {
      category: Category::Other,
      cursor: None,
      data: PlannerData::default(),
      detail_pane: PaneDrag::with_min_width(
        DETAIL_PANE_DEFAULT_WIDTH,
        DETAIL_PANE_MIN_WIDTH,
        crate::ui::style::spacing::layout::WINDOW_DEFAULT_WIDTH,
      )
      .right_anchored(true),
      facility_picker: None,
      loaded: false,
      menu: None,
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
      tree: NodeConfig::default(),
    }
  }

  pub fn apply_data(&mut self, data: PlannerData) {
    self.data = data;
    self.loaded = true;
    self.placeholder = format!(
      "Search {} buildable products\u{2026}",
      view::fmt_num(self.data.catalog.len() as i64)
    );
    if self.recent.is_empty() {
      self.recent = self.seed_recent();
    }
    if self.product.is_none()
      && let Some(first) = self.recent.first().copied()
    {
      self.select_product(first);
    }
  }

  pub fn category(&self) -> Category {
    self.category
  }

  pub fn cost_index(&self, path: &[i64], type_id: i64) -> Option<f64> {
    let is_reaction = self.data.recipe(type_id).is_some_and(|recipe| recipe.is_reaction);
    self
      .selected_facility(path, is_reaction)
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

  /// Whole-plan economics: `material_cost` is the rolled-up acquisition total of every raw input the
  /// bill of materials says you must buy, plus the install fees of any in-house sub-builds. With no
  /// component broken down this equals pricing the root recipe's materials; once a component is built
  /// in-house it equals buying that component's constituent parts plus its sub-job fee. Net profit and
  /// margin derive from this true cost so they match the bill of materials.
  pub fn economics(&self) -> Option<Economics> {
    let product = self.product?;
    let recipe = self.data.recipe(product)?;
    let plan = self.plan()?;

    let material_cost = self.plan_material_cost(&plan, &|path, type_id| self.cost_index(path, type_id).unwrap_or(0.0));

    let output_qty = recipe.output_per_run * self.runs;
    let revenue = self.data.price(product) * output_qty as f64;
    let install_fee = revenue * self.cost_index(&[], product).unwrap_or(0.0) * INSTALL_FEE_RATE;
    let profit = revenue - material_cost - install_fee;
    let margin = if revenue > 0.0 { profit / revenue * 100.0 } else { 0.0 };
    let per_unit = if output_qty > 0 {
      profit / output_qty as f64
    } else {
      0.0
    };
    let build_time_secs = node_build_time(recipe, self.runs, self.tree.te);

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

  pub fn menu(&self) -> Option<&MaterialMenu> {
    self.menu.as_ref()
  }

  pub fn node(&self, path: &[i64]) -> &NodeConfig {
    self.tree.at(path).unwrap_or(&self.tree)
  }

  pub fn picker_open(&self) -> bool {
    self.picker_open
  }

  pub fn picker_scroll_offset(&self) -> f32 {
    self.picker_scroll_offset
  }

  pub fn plan(&self) -> Option<BuildPlan> {
    let product = self.product?;
    let root = self.assemble(product, &self.tree)?;
    Some(BuildPlan::new(root, self.runs))
  }

  pub fn product(&self) -> Option<i64> {
    self.product
  }

  pub fn recent(&self) -> &[i64] {
    &self.recent
  }

  pub fn restore(&mut self, tree: &PlanTree) {
    self.product = Some(tree.product_type_id);
    self.set_runs(tree.runs);
    self.tree = NodeConfig::rebuild(&tree.nodes);
    self.facility_picker = None;
    self.push_recent(tree.product_type_id);
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

  pub fn set_pane_host_width(&mut self, host_width: f32) {
    self.detail_pane.set_host_width(host_width);
  }

  /// Returns the facility pinned to this node, falling back to the cheapest available
  /// default when the stored system id is absent from the current data (e.g. after a reload).
  pub fn selected_facility(&self, path: &[i64], is_reaction: bool) -> Option<&PlannerFacility> {
    match self.node(path).facility_system {
      Some(system) => self
        .data
        .facilities
        .iter()
        .find(|f| f.solar_system_id == system)
        .or_else(|| self.default_facility(is_reaction)),
      None => self.default_facility(is_reaction),
    }
  }

  pub fn shopping_list(&self) -> String {
    let Some(plan) = self.plan() else {
      return String::new();
    };
    let mut totals = plan.raw_totals();
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
    let mut nodes = Vec::new();
    self.tree.flatten(&mut Vec::new(), &mut nodes);
    Some(PlanTree {
      nodes,
      product_type_id: product,
      root_facility_system: self.tree.facility_system,
      runs: self.runs,
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
    match message {
      Message::CategorySelected(category) => {
        self.category = category;
        self.picker_scroll_offset = 0.0;
      }
      Message::CursorMoved(point) => self.cursor = Some(point),
      Message::FacilityPickerToggled {
        path,
      } => self.toggle_facility_picker(path),
      Message::FacilitySearchChanged {
        path,
        query,
      } => match self.facility_picker.as_mut().filter(|state| state.path == path) {
        Some(state) => state.query = query,
        // Typing into the always-visible field opens the picker for that node.
        None => {
          self.facility_picker = Some(FacilityPickerState {
            anchor: self.cursor.unwrap_or_default(),
            path,
            query,
          })
        }
      },
      Message::FacilitySelected {
        path,
        solar_system_id,
      } => {
        if let Some(node) = self.tree.at_mut(&path) {
          node.facility_system = Some(solar_system_id);
        }
        self.facility_picker = None;
      }
      Message::MaterialEfficiencyChanged {
        me,
        path,
      } => {
        if let Some(node) = self.tree.at_mut(&path) {
          node.me = me.clamp(0, ME_MAX);
        }
      }
      Message::MaterialRightPressed {
        mat,
        parent,
      } => self.open_menu(parent, mat),
      Message::MenuClosed => self.menu = None,
      Message::NodeBrokenDown {
        mat,
        parent,
      } => {
        self.break_down(&parent, mat);
        self.menu = None;
      }
      Message::NodeCollapsed {
        mat,
        parent,
      } => {
        if let Some(node) = self.tree.at_mut(&parent) {
          node.children.remove(&mat);
        }
        self.menu = None;
      }
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
      Message::RunsChanged(runs) => self.set_runs(runs),
      Message::RunsInputChanged(raw) => self.edit_runs(raw),
      Message::SearchChanged(query) => {
        self.search = query;
        self.picker_scroll_offset = 0.0;
      }
      // Clipboard write is handled by the parent industry::update; nothing to do here.
      Message::ShoppingListCopied => {}
      Message::TimeEfficiencyChanged {
        path,
        te,
      } => {
        if let Some(node) = self.tree.at_mut(&path) {
          node.te = te.clamp(0, TE_MAX);
        }
      }
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

  fn assemble(&self, type_id: i64, config: &NodeConfig) -> Option<BuildNode> {
    let recipe = self.data.recipe(type_id)?;
    let materials: Vec<Material> = recipe.materials.clone();
    let mut node = BuildNode::new(type_id, recipe.output_per_run, recipe.is_reaction, materials);
    node.me = if recipe.is_reaction { 0 } else { config.me };
    node.te = if recipe.is_reaction { 0 } else { config.te };
    for (&mat, child_config) in &config.children {
      if let Some(child) = self.assemble(mat, child_config) {
        node.children.insert(mat, child);
      }
    }
    Some(node)
  }

  fn break_down(&mut self, parent: &[i64], mat: i64) {
    if self.data.recipe(mat).is_none() {
      return;
    }
    let config = self.fresh_node(mat);
    if let Some(node) = self.tree.at_mut(parent) {
      node.children.entry(mat).or_insert(config);
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

  fn fresh_node(&self, type_id: i64) -> NodeConfig {
    let is_reaction = self.data.recipe(type_id).is_some_and(|recipe| recipe.is_reaction);
    let owned = self.data.owned.get(&type_id);
    let (me, te) = if is_reaction {
      (0, 0)
    } else {
      (
        owned.map(|bp| bp.material_efficiency).unwrap_or(DEFAULT_ME),
        owned.map(|bp| bp.time_efficiency).unwrap_or(DEFAULT_TE),
      )
    };
    NodeConfig {
      children: BTreeMap::new(),
      facility_system: None,
      me,
      te,
    }
  }

  fn open_menu(&mut self, parent: Vec<i64>, mat: i64) {
    let Some(anchor) = self.cursor else {
      return;
    };
    let buildable = self.data.recipe(mat).is_some();
    let built = self
      .tree
      .at(&parent)
      .is_some_and(|node| node.children.contains_key(&mat));
    self.menu = Some(MaterialMenu {
      anchor,
      buildable,
      built,
      mat,
      parent,
    });
  }

  /// Total acquisition cost of a build plan: every raw input priced at market plus the install fee of
  /// each in-house sub-build. `cost_index` resolves a node's facility cost index (0.0 when none).
  fn plan_material_cost(&self, plan: &BuildPlan, cost_index: &dyn Fn(&[i64], i64) -> f64) -> f64 {
    let acquisition: f64 = plan
      .raw_totals()
      .iter()
      .map(|total| total.qty as f64 * self.data.price(total.type_id))
      .sum();

    let sub_fees: f64 = plan
      .build_order()
      .iter()
      .filter(|job| !job.path.is_empty())
      .map(|job| {
        let produced = job.node.output_per_run * job.runs;
        self.data.price(job.type_id) * produced as f64 * cost_index(&job.path, job.type_id) * INSTALL_FEE_RATE
      })
      .sum();

    acquisition + sub_fees
  }

  fn push_recent(&mut self, type_id: i64) {
    self.recent.retain(|&id| id != type_id);
    self.recent.insert(0, type_id);
    self.recent.truncate(RECENT_LIMIT);
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
    self.tree = self.fresh_node(type_id);
    self.facility_picker = None;
  }

  fn set_runs(&mut self, runs: i64) {
    self.runs = runs.clamp(1, RUNS_MAX);
    self.runs_input = self.runs.to_string();
  }

  fn toggle_facility_picker(&mut self, path: Vec<i64>) {
    if self.facility_picker.as_ref().is_some_and(|state| state.path == path) {
      self.facility_picker = None;
    } else {
      self.facility_picker = Some(FacilityPickerState {
        anchor: self.cursor.unwrap_or_default(),
        path,
        query: String::new(),
      });
    }
  }

  /// Whole-plan economics for a saved plan, recomputed at current prices. Mirrors [`Planner::economics`]
  /// but reads the product, runs, and per-node ME/TE/facility from `tree` instead of live state, so a list
  /// of saved plans reflects today's market without rehydrating each into the live planner.
  fn tree_economics(&self, tree: &PlanTree) -> Option<Economics> {
    let product = tree.product_type_id;
    let recipe = self.data.recipe(product)?;
    let runs = tree.runs.clamp(1, RUNS_MAX);
    let root = tree.nodes.iter().find(|node| node.path.is_empty());
    let te = root.map(|node| node.te).unwrap_or(0);

    let config = NodeConfig::rebuild(&tree.nodes);
    let plan = BuildPlan::new(self.assemble(product, &config)?, runs);
    let material_cost = self.plan_material_cost(&plan, &|path, type_id| {
      let is_reaction = self.data.recipe(type_id).is_some_and(|recipe| recipe.is_reaction);
      self.cost_index_for(config.at(path).and_then(|node| node.facility_system), is_reaction)
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
          mat: menu.mat,
          parent: menu.parent.clone(),
        },
      ));
    } else {
      items.push(context_menu::Item::warning(
        "Break down \u{2014} build in-house",
        Message::NodeBrokenDown {
          mat: menu.mat,
          parent: menu.parent.clone(),
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
        path: state.path.clone(),
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
    clients::eve_image::Size,
    features::industry::{
      planner_loaders::{Category, PlannerData, PlannerFacility, Recipe},
      planner_model::{SubBuild, eff_qty, runs_for},
    },
    store::images::{self, IconResolution},
    ui::{
      components::{
        badge::badge,
        clip::clip_layer,
        icon::Icon,
        icon_tile::icon_tile,
        resizable_pane::pane_handle,
        rule,
        text_input::{TextInput, inner_style as text_input_inner_style},
        virtual_list::{self, VirtualList, VirtualListConfig},
      },
      style::{color, radius, spacing, typography},
    },
  };

  const ESTIMATED_PICKER_ROW: f32 = 52.0;
  const FACILITY_PICKER_GAP: f32 = 6.0;
  const FACILITY_PICKER_LIST_HEIGHT: f32 = 230.0;
  const FACILITY_PICKER_WIDTH: f32 = 320.0;
  const PANE_PADDING: f32 = 24.0;
  const PICKER_MAX_RESULTS: usize = 200;
  const RUNS_FIELD_WIDTH: f32 = 34.0;
  const RUNS_STEPPER_HEIGHT: f32 = 34.0;
  const RUNS_STEP_WIDTH: f32 = 30.0;
  const TILE_BOX: f32 = 30.0;
  /// Must be S64 — `resolve_type_icon` only returns a bundled icon at this size; smaller sizes fall back to a placeholder glyph.
  const TILE_ICON: Size = Size::S64;
  const TREE_INDENT: f32 = 22.0;

  const COL_COST: f32 = 140.0;
  const COL_PRICE: f32 = 120.0;
  const COL_QTY: f32 = 120.0;

  struct MaterialLine {
    building: bool,
    cost: f64,
    depth: usize,
    qty: i64,
    unit: f64,
  }

  pub(super) fn body(planner: &Planner) -> Element<'_, Message> {
    let Some(product) = planner.product() else {
      return empty();
    };
    let Some(recipe) = planner.data().recipe(product) else {
      return empty();
    };

    let left = scrollable(left_pane(planner, product, recipe))
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
    let plan = planner.plan();
    let steps = plan.as_ref().map(|plan| plan.node_count()).unwrap_or(1);

    let mut children: Vec<Element<'a, Message>> = vec![
      picker(planner),
      section_label("Blueprints", (steps > 1).then(|| format!("{steps} steps"))),
      blueprint_card(planner, product, recipe, &[], None),
    ];

    if let Some(plan) = plan.as_ref() {
      for sub in plan.collect_builds() {
        children.push(sub_blueprint_card(planner, sub));
      }
    }

    let me_hint = if recipe.is_reaction {
      "reaction inputs".to_owned()
    } else {
      format!("ME {} applied", planner.node(&[]).me)
    };
    children.push(section_label(
      "Material plan",
      Some(format!("{me_hint} \u{00B7} right-click an item to break it down")),
    ));
    children.push(material_plan(planner, recipe));

    if let Some(plan) = plan.as_ref() {
      children.push(bill_of_materials(planner, plan));
      children.push(build_order(planner, plan));
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

    if !planner.picker_open() && planner.search().is_empty() {
      return bar;
    }

    Column::with_children(vec![bar, picker_results(planner)])
      .spacing(spacing::SPACE_2)
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

    let row = Row::with_children(vec![type_tile(type_id), details.into(), badges.into()])
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

  fn blueprint_card<'a>(
    planner: &'a Planner,
    type_id: i64,
    recipe: &'a Recipe,
    path: &[i64],
    sub: Option<&SubBuild>,
  ) -> Element<'a, Message> {
    let config = planner.node(path);
    let is_reaction = recipe.is_reaction;
    let runs = sub.map(|sub| sub.runs).unwrap_or_else(|| planner.runs());

    let header = blueprint_header(planner, type_id, is_reaction, sub);

    // Runs sit on the left, the ME/TE sliders are centered, and the location search is floated right.
    let mut center: Vec<Element<'a, Message>> = Vec::new();
    if !is_reaction {
      center.push(efficiency_slider(
        "Material efficiency",
        config.me,
        super::ME_MAX,
        path.to_vec(),
        true,
      ));
      center.push(efficiency_slider(
        "Time efficiency",
        config.te,
        super::TE_MAX,
        path.to_vec(),
        false,
      ));
    }

    let controls = Row::with_children(vec![
      runs_control(runs, planner.runs_input(), sub.is_some(), is_reaction),
      Space::new().width(Length::Fill).into(),
      Row::with_children(center)
        .spacing(spacing::SPACE_6)
        .align_y(Vertical::Top)
        .into(),
      Space::new().width(Length::Fill).into(),
      facility_control(planner, path, is_reaction),
    ])
    .spacing(spacing::SPACE_6)
    .align_y(Vertical::Top)
    .width(Length::Fill);

    let mut body: Vec<Element<'a, Message>> = vec![header, controls.into()];

    if let Some(sub) = sub {
      body.push(rule::horizontal());
      body.push(build_vs_buy(planner, type_id, recipe, sub));
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

  fn sub_blueprint_card<'a>(planner: &'a Planner, sub: SubBuild) -> Element<'a, Message> {
    let Some(recipe) = planner.data().recipe(sub.type_id) else {
      return Space::new().into();
    };
    let indent = (sub.depth.saturating_sub(1)) as f32 * 18.0;
    container(blueprint_card(planner, sub.type_id, recipe, &sub.path, Some(&sub)))
      .padding(Padding {
        left: indent,
        ..Padding::ZERO
      })
      .into()
  }

  fn blueprint_header<'a>(
    planner: &'a Planner,
    type_id: i64,
    is_reaction: bool,
    sub: Option<&SubBuild>,
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
    badges = if sub.is_some() {
      badges.push(badge("BUILDING", Some(color::status::WARNING)))
    } else {
      badges.push(owned_badge(planner, type_id))
    };

    let subtitle = match sub {
      Some(sub) => format!(
        "builds {} \u{00B7} needs {} for parent job",
        fmt_num(data.recipe(type_id).map(|r| r.output_per_run).unwrap_or(1) * sub.runs),
        fmt_num(sub.needed_qty)
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

    let mut row = Row::with_children(vec![type_tile(type_id), details.into()])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .width(Length::Fill);

    if let Some(sub) = sub {
      let parent = sub.path[..sub.path.len() - 1].to_vec();
      let mat = sub.type_id;
      row = row.push(
        button(
          Icon::close()
            .color(color::text::tertiary())
            .size(14.0)
            .render::<Message>(),
        )
        .padding(spacing::UNIT)
        .on_press(Message::NodeCollapsed {
          mat,
          parent,
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

  fn efficiency_slider<'a>(label: &str, value: i64, max: i64, path: Vec<i64>, material: bool) -> Element<'a, Message> {
    let prefix = if material { "ME" } else { "TE" };
    let handle = move |next: f64| {
      let next = next.round() as i64;
      if material {
        Message::MaterialEfficiencyChanged {
          me: next,
          path: path.clone(),
        }
      } else {
        Message::TimeEfficiencyChanged {
          path: path.clone(),
          te: next,
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

  fn facility_control<'a>(planner: &'a Planner, path: &[i64], is_reaction: bool) -> Element<'a, Message> {
    let selected = planner.selected_facility(path, is_reaction);
    let open = planner.facility_picker().is_some_and(|state| state.path == path);

    let query = if open {
      planner
        .facility_picker()
        .map(|state| state.query.as_str())
        .unwrap_or("")
    } else {
      ""
    };

    let placeholder: &'a str = match selected {
      Some(facility) => facility.name.as_str(),
      None => "No facilities available",
    };

    let owned_path = path.to_vec();
    let input = TextInput::new(placeholder, query, move |value| Message::FacilitySearchChanged {
      path: owned_path.clone(),
      query: value,
    })
    .leading_icon(Icon::search().color(if open {
      color::accent::PLASMA
    } else {
      color::text::secondary()
    }))
    .background(color::surface::SUNKEN)
    .width(Length::Fill)
    .render();

    let percent: Element<'a, Message> = match selected {
      Some(facility) => text(format!(
        "{:.2}% index",
        facility.index_for(is_reaction).unwrap_or(0.0) * 100.0
      ))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()))
      .into(),
      None => Space::new().into(),
    };

    Column::with_children(vec![
      micro_label("Build at"),
      container(input).width(Length::Fill).into(),
      percent,
    ])
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
    let path = state.path.as_slice();
    let is_reaction = planner
      .product()
      .filter(|_| path.is_empty())
      .and_then(|product| planner.data().recipe(product))
      .map(|recipe| recipe.is_reaction)
      .or_else(|| {
        path
          .last()
          .and_then(|&type_id| planner.data().recipe(type_id))
          .map(|recipe| recipe.is_reaction)
      })
      .unwrap_or(false);
    let needle = state.query.trim().to_lowercase();
    let selected_system = planner.selected_facility(path, is_reaction).map(|f| f.solar_system_id);

    let rows: Vec<Element<'_, Message>> = planner
      .data()
      .facilities
      .iter()
      .filter(|facility| facility.index_for(is_reaction).is_some())
      .filter(|facility| {
        needle.is_empty()
          || facility.name.to_lowercase().contains(&needle)
          || facility.solar_system_id.to_string().contains(&needle)
      })
      .map(|facility| facility_row(path, facility, is_reaction, selected_system))
      .collect();

    let list: Element<'_, Message> = if rows.is_empty() {
      centered(
        text("No facilities match.")
          .font(typography::body::REGULAR)
          .size(typography::size::SM)
          .style(typography::colored(color::text::tertiary())),
      )
    } else {
      scrollable(Column::with_children(rows).spacing(spacing::UNIT).width(Length::Fill))
        .style(crate::ui::style::control::scrollbar)
        .width(Length::Fill)
        .height(Length::Fixed(FACILITY_PICKER_LIST_HEIGHT))
        .into()
    };

    let panel = container(list)
      .width(Length::Fixed(FACILITY_PICKER_WIDTH))
      .padding(spacing::SPACE_2)
      .style(|_| container::Style {
        background: Some(Background::Color(color::surface::RAISED)),
        border: Border {
          color: color::rule_strong(),
          radius: radius::CARD.into(),
          width: 1.0,
        },
        shadow: crate::ui::style::shadow::CARD,
        ..container::Style::default()
      });

    // Right padding clears the detail pane and the card/pane gutters so the panel's right edge lines up with
    // the right-floated facility input rather than the planner's right edge.
    let right = planner.detail_pane_width() + PANE_PADDING + spacing::SPACE_3_5;
    crate::ui::components::positioned_dropdown::positioned_dropdown_right(panel.into(), anchor_top, right)
  }

  fn facility_row<'a>(
    path: &[i64],
    facility: &'a PlannerFacility,
    is_reaction: bool,
    selected_system: Option<i64>,
  ) -> Element<'a, Message> {
    let on = selected_system == Some(facility.solar_system_id);
    let pct = facility.index_for(is_reaction).unwrap_or(0.0) * 100.0;

    let name = text(facility.name.clone())
      .font(typography::body::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(if on {
        color::accent::PLASMA
      } else {
        color::text::PRIMARY
      }));
    let system = text(facility.solar_system_id.to_string())
      .font(typography::mono::REGULAR)
      .size(typography::size::XS)
      .style(typography::colored(color::text::tertiary()));

    let details = Column::with_children(vec![name.into(), system.into()])
      .spacing(spacing::UNIT)
      .width(Length::Fill);

    let pct_label = text(format!("{pct:.2}%"))
      .font(typography::mono::REGULAR)
      .size(typography::size::XS_PLUS)
      .style(typography::colored(color::text::secondary()));

    let row = Row::with_children(vec![details.into(), pct_label.into()])
      .spacing(spacing::SPACE_2_5)
      .align_y(Vertical::Center)
      .width(Length::Fill);

    button(row)
      .width(Length::Fill)
      .padding(Padding {
        top: spacing::SPACE_2,
        bottom: spacing::SPACE_2,
        left: spacing::SPACE_2_5,
        right: spacing::SPACE_2_5,
      })
      .on_press(Message::FacilitySelected {
        path: path.to_vec(),
        solar_system_id: facility.solar_system_id,
      })
      .style(move |_, _| button::Style {
        background: on.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.12))),
        border: Border {
          radius: radius::CONTROL.into(),
          ..Border::default()
        },
        text_color: color::text::PRIMARY,
        ..button::Style::default()
      })
      .into()
  }

  fn build_vs_buy<'a>(planner: &'a Planner, type_id: i64, recipe: &'a Recipe, sub: &SubBuild) -> Element<'a, Message> {
    let data = planner.data();
    let config = planner.node(&sub.path);
    let material_cost: f64 = recipe
      .materials
      .iter()
      .map(|m| eff_qty(m.base_qty, sub.runs, config.me, recipe.is_reaction) as f64 * data.price(m.type_id))
      .sum();
    let produced = recipe.output_per_run * sub.runs;
    let fee = data.price(type_id)
      * produced as f64
      * planner.cost_index(&sub.path, type_id).unwrap_or(0.0)
      * super::INSTALL_FEE_RATE;
    let build_cost = material_cost + fee;
    let buy_cost = sub.needed_qty as f64 * data.price(type_id);
    let savings = buy_cost - build_cost;
    let build_time = node_build_time(recipe, sub.runs, config.te);

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
      metric("Build time", &fmt_duration(build_time), color::text::secondary()),
    ])
    .spacing(spacing::SPACE_6)
    .into()
  }

  fn material_plan<'a>(planner: &'a Planner, recipe: &'a Recipe) -> Element<'a, Message> {
    let mut rows: Vec<Element<'a, Message>> = vec![grid_header()];
    let mut total = 0.0;
    material_rows(planner, recipe, planner.runs(), &[], 0, &mut rows, &mut total);

    rows.push(footer_row("Material cost", &fmt_isk_full(total)));

    container(Column::with_children(rows).width(Length::Fill))
      .width(Length::Fill)
      .style(bordered_table)
      .into()
  }

  fn material_rows<'a>(
    planner: &'a Planner,
    recipe: &'a Recipe,
    runs: i64,
    path: &[i64],
    depth: usize,
    out: &mut Vec<Element<'a, Message>>,
    total: &mut f64,
  ) {
    let data = planner.data();
    let config = planner.node(path);
    for material in &recipe.materials {
      let qty = eff_qty(material.base_qty, runs, config.me, recipe.is_reaction);
      let unit = data.price(material.type_id);
      let cost = qty as f64 * unit;
      let child = config.children.contains_key(&material.type_id);
      if depth == 0 {
        *total += cost;
      }

      out.push(material_row(
        planner,
        material.type_id,
        MaterialLine {
          building: child,
          cost,
          depth,
          qty,
          unit,
        },
        path,
      ));

      if child && let Some(child_recipe) = data.recipe(material.type_id) {
        let child_runs = runs_for(qty, child_recipe.output_per_run);
        let mut child_path = path.to_vec();
        child_path.push(material.type_id);
        material_rows(planner, child_recipe, child_runs, &child_path, depth + 1, out, total);
      }
    }
  }

  fn material_row<'a>(planner: &'a Planner, type_id: i64, line: MaterialLine, parent: &[i64]) -> Element<'a, Message> {
    let MaterialLine {
      building,
      cost,
      depth,
      qty,
      unit,
    } = line;
    let data = planner.data();
    let buildable = data.recipe(type_id).is_some();

    let mut name_row = Row::with_children(vec![type_tile(type_id)])
      .spacing(spacing::SPACE_2)
      .align_y(Vertical::Center);
    name_row = name_row.push(
      text(data.name(type_id))
        .font(typography::body::REGULAR)
        .size(typography::size::MD)
        .style(typography::colored(color::text::PRIMARY)),
    );
    if building {
      name_row = name_row.push(badge("BUILDING", Some(color::status::WARNING)));
    } else if buildable {
      name_row = name_row.push(
        text("buildable")
          .font(typography::mono::REGULAR)
          .size(typography::size::XS)
          .style(typography::colored(color::text::tertiary())),
      );
    }

    let name_cell = container(name_row).padding(Padding {
      left: depth as f32 * TREE_INDENT,
      ..Padding::ZERO
    });

    let grid = Row::with_children(vec![
      container(name_cell).width(Length::Fill).into(),
      grid_value(&fmt_num(qty), COL_QTY, color::text::PRIMARY),
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

    let background = if building {
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
      mat: type_id,
      parent: parent.to_vec(),
    });
    if buildable && !building {
      area = area.on_press(Message::NodeBrokenDown {
        mat: type_id,
        parent: parent.to_vec(),
      });
    }
    area.into()
  }

  fn bill_of_materials<'a>(planner: &'a Planner, plan: &super::BuildPlan) -> Element<'a, Message> {
    let data = planner.data();
    let mut totals = plan.raw_totals();
    totals.sort_by(|a, b| (b.qty as f64 * data.price(b.type_id)).total_cmp(&(a.qty as f64 * data.price(a.type_id))));
    let acquisition: f64 = totals.iter().map(|t| t.qty as f64 * data.price(t.type_id)).sum();

    let mut rows: Vec<Element<'a, Message>> = vec![grid_header()];
    for total in &totals {
      let unit = data.price(total.type_id);
      let cost = total.qty as f64 * unit;
      rows.push(
        container(
          Row::with_children(vec![
            container(
              Row::with_children(vec![
                type_tile(total.type_id),
                text(data.name(total.type_id))
                  .font(typography::body::REGULAR)
                  .size(typography::size::MD)
                  .style(typography::colored(color::text::PRIMARY))
                  .into(),
              ])
              .spacing(spacing::SPACE_2)
              .align_y(Vertical::Center),
            )
            .width(Length::Fill)
            .into(),
            grid_value(&fmt_num(total.qty), COL_QTY, color::text::PRIMARY),
            grid_value(&fmt_price(unit), COL_PRICE, color::text::secondary()),
            grid_value(&fmt_isk(cost), COL_COST, color::text::PRIMARY),
          ])
          .spacing(spacing::SPACE_3)
          .align_y(Vertical::Center)
          .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding(Padding {
          top: spacing::SPACE_2_5,
          bottom: spacing::SPACE_2_5,
          left: spacing::SPACE_3,
          right: spacing::SPACE_3,
        })
        .style(|_| container::Style {
          border: Border {
            color: color::rule(),
            radius: 0.0.into(),
            width: 1.0,
          },
          ..container::Style::default()
        })
        .into(),
      );
    }
    rows.push(footer_row("Acquisition cost", &fmt_isk_full(acquisition)));

    Column::with_children(vec![
      section_label(
        "Bill of materials",
        Some(format!("raw inputs to acquire \u{00B7} {} items", totals.len())),
      ),
      container(Column::with_children(rows).width(Length::Fill))
        .width(Length::Fill)
        .style(bordered_table)
        .into(),
    ])
    .spacing(spacing::SPACE_3)
    .into()
  }

  fn build_order<'a>(planner: &'a Planner, plan: &super::BuildPlan) -> Element<'a, Message> {
    let data = planner.data();
    let jobs = plan.build_order();
    let count = jobs.len();

    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for (index, job) in jobs.iter().enumerate() {
      let is_final = job.path.is_empty();
      let time = node_build_time(&recipe_for(data, job.type_id), job.runs, job.node.te);
      let parent_name = if is_final {
        "final product".to_owned()
      } else {
        let parent = if job.path.len() >= 2 {
          job.path[job.path.len() - 2]
        } else {
          plan.root.type_id
        };
        format!("feeds \u{2192} {}", data.name(parent))
      };

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
        type_tile(job.type_id),
        Column::with_children(vec![
          Row::with_children(vec![
            text(data.name(job.type_id))
              .font(typography::body::MEDIUM)
              .size(typography::size::MD)
              .style(typography::colored(color::text::PRIMARY))
              .into(),
            activity_badge(job.node.is_reaction),
            text(format!(
              "\u{00D7}{} {}",
              fmt_num(job.runs),
              if job.node.is_reaction { "cycles" } else { "runs" }
            ))
            .font(typography::mono::REGULAR)
            .size(typography::size::XS_PLUS)
            .style(typography::colored(color::text::secondary()))
            .into(),
          ])
          .spacing(spacing::SPACE_2)
          .align_y(Vertical::Center)
          .into(),
          text(parent_name)
            .font(typography::mono::REGULAR)
            .size(typography::size::XS_PLUS)
            .style(typography::colored(color::text::tertiary()))
            .into(),
        ])
        .spacing(spacing::UNIT)
        .width(Length::Fill)
        .into(),
        text(fmt_duration(time))
          .font(typography::mono::REGULAR)
          .size(typography::size::MD)
          .style(typography::colored(color::text::PRIMARY))
          .into(),
      ])
      .spacing(spacing::SPACE_3)
      .align_y(Vertical::Center)
      .width(Length::Fill);

      let highlight = is_final;
      rows.push(
        container(body)
          .width(Length::Fill)
          .padding(Padding {
            top: spacing::SPACE_3,
            bottom: spacing::SPACE_3,
            left: spacing::SPACE_3,
            right: spacing::SPACE_3,
          })
          .style(move |_| container::Style {
            background: highlight.then(|| Background::Color(color::with_alpha(color::accent::PLASMA, 0.07))),
            border: Border {
              color: color::rule(),
              radius: 0.0.into(),
              width: 1.0,
            },
            ..container::Style::default()
          })
          .into(),
      );
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

  fn right_pane<'a>(planner: &'a Planner, product: i64) -> Element<'a, Message> {
    let tabs = Row::with_children(vec![
      right_tab_button("Detail", RightTab::Detail, planner.right_tab()),
      right_tab_button("Plans", RightTab::Plans, planner.right_tab()),
    ])
    .spacing(spacing::SPACE_3);

    let content: Element<'a, Message> = match planner.right_tab() {
      RightTab::Detail => detail_pane(planner, product),
      RightTab::Plans => plans_pane(planner),
    };

    let column = Column::with_children(vec![
      container(tabs)
        .width(Length::Fill)
        .padding(Padding {
          top: 0.0,
          bottom: 0.0,
          left: spacing::SPACE_3,
          right: spacing::SPACE_3,
        })
        .into(),
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
      type_tile(product),
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
      meta_line(Icon::clock(), "Build time", &fmt_duration(eco.build_time_secs)),
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
      type_tile(plan.product_type_id),
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

  fn right_tab_button<'a>(label: &str, tab: RightTab, active: RightTab) -> Element<'a, Message> {
    let on = tab == active;
    button(
      text(label.to_owned())
        .font(typography::body::MEDIUM)
        .size(typography::size::MD)
        .style(typography::colored(if on {
          color::text::PRIMARY
        } else {
          color::text::secondary()
        })),
    )
    .padding(Padding {
      top: spacing::SPACE_3,
      bottom: spacing::SPACE_3,
      left: spacing::SPACE_2,
      right: spacing::SPACE_2,
    })
    .on_press(Message::RightTabSelected(tab))
    .style(move |_, _| button::Style {
      border: Border {
        color: if on {
          color::accent::PLASMA
        } else {
          iced::Color::TRANSPARENT
        },
        radius: 0.0.into(),
        width: 0.0,
      },
      text_color: if on {
        color::text::PRIMARY
      } else {
        color::text::secondary()
      },
      ..button::Style::default()
    })
    .into()
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

  fn type_tile<'a>(type_id: i64) -> Element<'a, Message> {
    match images::default_store().resolve_type_icon(type_id, None, TILE_ICON) {
      IconResolution::Found(path) => icon_tile(
        clip_layer(
          image(image::Handle::from_path(path))
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

  fn empty<'a>() -> Element<'a, Message> {
    centered(
      text("Search a product to start planning a build.")
        .font(typography::body::REGULAR)
        .size(typography::size::LG)
        .style(typography::colored(color::text::tertiary())),
    )
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

  fn fmt_isk(value: f64) -> String {
    let v = value.abs();
    if v >= 1_000_000_000_000.0 {
      format!("{:.2}T", v / 1_000_000_000_000.0)
    } else if v >= 1_000_000_000.0 {
      format!("{:.2}B", v / 1_000_000_000.0)
    } else if v >= 1_000_000.0 {
      format!("{:.2}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
      format!("{:.1}K", v / 1_000.0)
    } else {
      format!("{v:.0}")
    }
  }

  fn fmt_isk_full(value: f64) -> String {
    fmt_num(value.round() as i64)
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

  fn fmt_duration(seconds: f64) -> String {
    let total = seconds.max(0.0) as i64;
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let minutes = (total % 3_600) / 60;
    if days > 0 {
      format!("{days}d {hours}h")
    } else if hours > 0 {
      format!("{hours}h {minutes}m")
    } else {
      format!("{minutes}m")
    }
  }

  fn fmt_volume(value: f64) -> String {
    format!("{} m\u{00B3}", fmt_num(value.round() as i64))
  }

  #[cfg(test)]
  mod tests {
    use super::*;
    use crate::store::images::Store;

    #[test]
    fn it_renders_type_tiles_at_the_bundled_icon_size() {
      assert_eq!(TILE_ICON, Size::S64);
    }

    #[test]
    fn it_resolves_a_bundled_icon_at_the_tile_size() {
      let data = tempfile::tempdir().unwrap();
      let committed = tempfile::tempdir().unwrap();
      let store = Store::new(data.path().to_path_buf()).with_committed_items(committed.path().to_path_buf());
      std::fs::write(committed.path().join("34.png"), [1]).unwrap();

      let resolved = store.resolve_type_icon(34, None, TILE_ICON);

      assert!(matches!(resolved, IconResolution::Found(_)));
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::features::industry::planner_loaders::{CatalogEntry, OwnedSummary};

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

  mod break_down {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_nests_a_buildable_child_with_runs_locked_to_parent_demand() {
      let mut planner = planner();
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        path: vec![],
      });

      planner.update(Message::NodeBrokenDown {
        mat: RETRIEVER,
        parent: vec![],
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
        mat: TRITANIUM,
        parent: vec![],
      });

      assert!(planner.node(&[]).children.is_empty());
    }

    #[test]
    fn it_rolls_a_broken_down_child_into_raw_totals() {
      let mut planner = planner();
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        path: vec![],
      });
      planner.update(Message::NodeBrokenDown {
        mat: RETRIEVER,
        parent: vec![],
      });
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        path: vec![RETRIEVER],
      });

      let totals = planner.plan().unwrap().raw_totals();
      let tritanium = totals.iter().find(|t| t.type_id == TRITANIUM).unwrap();
      // 5 direct + 2 retrievers × 10 = 25.
      assert_eq!(tritanium.qty, 25);
      assert!(totals.iter().all(|t| t.type_id != RETRIEVER));
    }
  }

  mod collapse {

    use super::*;

    #[test]
    fn it_restores_a_collapsed_child_to_a_raw_input() {
      let mut planner = planner();
      planner.update(Message::NodeBrokenDown {
        mat: RETRIEVER,
        parent: vec![],
      });

      planner.update(Message::NodeCollapsed {
        mat: RETRIEVER,
        parent: vec![],
      });

      let totals = planner.plan().unwrap().raw_totals();
      assert!(totals.iter().any(|t| t.type_id == RETRIEVER));
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
        mat: RETRIEVER,
        parent: vec![],
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
        mat: RETRIEVER,
        parent: vec![],
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
  }

  mod facility_picker {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_a_node_to_the_cheapest_eligible_facility() {
      let planner = planner();

      let facility = planner.selected_facility(&[], false).unwrap();

      assert_eq!(facility.name, "Cheap Citadel");
      assert_eq!(facility.solar_system_id, 30_002_187);
    }

    #[test]
    fn it_toggles_a_per_node_picker_open_and_closed() {
      let mut planner = planner();

      planner.update(Message::FacilityPickerToggled {
        path: vec![],
      });
      assert_eq!(planner.facility_picker().map(|state| state.path.clone()), Some(vec![]));

      planner.update(Message::FacilityPickerToggled {
        path: vec![],
      });
      assert!(planner.facility_picker().is_none());
    }

    #[test]
    fn it_anchors_the_popover_at_the_cursor_when_the_picker_opens() {
      let mut planner = planner();
      planner.update(Message::CursorMoved(Point::new(640.0, 215.0)));

      planner.update(Message::FacilityPickerToggled {
        path: vec![],
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
        path: vec![],
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
        mat: RETRIEVER,
        parent: vec![],
      });

      planner.update(Message::FacilityPickerToggled {
        path: vec![],
      });
      planner.update(Message::FacilityPickerToggled {
        path: vec![RETRIEVER],
      });

      assert_eq!(
        planner.facility_picker().map(|state| state.path.clone()),
        Some(vec![RETRIEVER])
      );
    }

    #[test]
    fn it_records_the_search_query_for_the_open_node() {
      let mut planner = planner();
      planner.update(Message::FacilityPickerToggled {
        path: vec![],
      });

      planner.update(Message::FacilitySearchChanged {
        path: vec![],
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
        mat: RETRIEVER,
        parent: vec![],
      });
      planner.update(Message::FacilityPickerToggled {
        path: vec![],
      });

      // Typing into a different node's field switches the open picker to that node.
      planner.update(Message::FacilitySearchChanged {
        path: vec![RETRIEVER],
        query: "amarr".to_owned(),
      });

      assert_eq!(
        planner.facility_picker().map(|state| state.path.clone()),
        Some(vec![RETRIEVER])
      );
      assert_eq!(
        planner.facility_picker().map(|state| state.query.clone()),
        Some("amarr".to_owned())
      );
    }

    #[test]
    fn it_pins_the_selected_facility_system_and_closes_the_picker() {
      let mut planner = planner();
      planner.update(Message::FacilityPickerToggled {
        path: vec![],
      });

      planner.update(Message::FacilitySelected {
        path: vec![],
        solar_system_id: 30_000_142,
      });

      assert_eq!(planner.node(&[]).facility_system, Some(30_000_142));
      assert_eq!(planner.selected_facility(&[], false).unwrap().name, "Pricey Station");
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

      assert_eq!(planner.node(&[]).me, 0);
      assert_eq!(planner.node(&[]).te, 0);
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

      assert_eq!(planner.node(&[]).me, 8);
      assert_eq!(planner.node(&[]).te, 16);
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
        path: vec![],
      });
      planner.update(Message::NodeBrokenDown {
        mat: RETRIEVER,
        parent: vec![],
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
        mat: RETRIEVER,
        parent: vec![],
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
        path: vec![],
      });
      planner.update(Message::NodeBrokenDown {
        mat: RETRIEVER,
        parent: vec![],
      });
      planner.update(Message::MaterialEfficiencyChanged {
        me: 0,
        path: vec![RETRIEVER],
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
        path: vec![],
      });
      planner.update(Message::TimeEfficiencyChanged {
        path: vec![],
        te: 14,
      });
      planner.update(Message::FacilitySelected {
        path: vec![],
        solar_system_id: 30_000_142,
      });
      planner.update(Message::NodeBrokenDown {
        mat: RETRIEVER,
        parent: vec![],
      });
      planner.update(Message::MaterialEfficiencyChanged {
        me: 4,
        path: vec![RETRIEVER],
      });
      planner.update(Message::FacilitySelected {
        path: vec![RETRIEVER],
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
    fn it_captures_the_root_and_per_node_configuration() {
      let snapshot = configured_planner().snapshot().unwrap();

      assert_eq!(snapshot.product_type_id, HULK);
      assert_eq!(snapshot.runs, 3);
      assert_eq!(snapshot.root_facility_system, Some(30_000_142));

      let root = snapshot.nodes.iter().find(|node| node.path.is_empty()).unwrap();
      assert_eq!((root.me, root.te), (7, 14));

      let child = snapshot.nodes.iter().find(|node| node.path == vec![RETRIEVER]).unwrap();
      assert_eq!((child.me, child.facility_system), (4, Some(30_002_187)));
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
  }
}
