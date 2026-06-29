use std::collections::{BTreeMap, HashMap};

use super::rig_bonuses::{DerivedRigBonuses, RigBonus, derive_rig_bonuses};

/// ESI activity id for reactions in pod (the design reference used a synthetic 9). Reaction nodes ignore ME.
pub const REACTION_ACTIVITY_ID: i64 = 11;

#[derive(Clone, Debug, PartialEq)]
pub struct BuildJob {
  pub needed_qty: i64,
  pub node: BuildNode,
  pub path: Vec<i64>,
  pub runs: i64,
  pub type_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuildNode {
  pub children: BTreeMap<i64, BuildNode>,
  pub facility: Option<i64>,
  pub facility_structure: Option<i64>,
  pub is_reaction: bool,
  pub materials: Vec<Material>,
  pub me: i64,
  pub output_per_run: i64,
  pub rig_fee_factor: f64,
  pub rig_me_factor: f64,
  pub rig_te_factor: f64,
  pub te: i64,
  pub type_id: i64,
}

impl BuildNode {
  pub fn new(type_id: i64, output_per_run: i64, is_reaction: bool, materials: Vec<Material>) -> Self {
    BuildNode {
      children: BTreeMap::new(),
      facility: None,
      facility_structure: None,
      is_reaction,
      materials,
      me: 0,
      output_per_run: output_per_run.max(1),
      rig_fee_factor: 1.0,
      rig_me_factor: 1.0,
      rig_te_factor: 1.0,
      te: 0,
      type_id,
    }
  }

  pub fn count_nodes(&self) -> usize {
    1 + self.children.values().map(BuildNode::count_nodes).sum::<usize>()
  }

  pub fn runs_for(&self, needed_qty: i64) -> i64 {
    runs_for(needed_qty, self.output_per_run)
  }

  fn collect_into(&self, runs: i64, path: &[i64], depth: usize, out: &mut Vec<SubBuild>) {
    for (&mat, child) in &self.children {
      let needed = self.needed_for(mat, runs);
      let child_runs = child.runs_for(needed);
      let mut child_path = path.to_vec();
      child_path.push(mat);

      out.push(SubBuild {
        depth: depth + 1,
        needed_qty: needed,
        node: child.clone(),
        path: child_path.clone(),
        runs: child_runs,
        type_id: mat,
      });

      child.collect_into(child_runs, &child_path, depth + 1, out);
    }
  }

  fn needed_for(&self, mat: i64, runs: i64) -> i64 {
    self
      .materials
      .iter()
      .find(|m| m.type_id == mat)
      .map(|m| eff_qty(m.base_qty, runs, self.me, self.is_reaction, self.rig_me_factor))
      .unwrap_or(0)
  }

  fn order_into(&self, needed_qty: i64, runs: i64, path: &[i64], out: &mut Vec<BuildJob>) {
    for (&mat, child) in &self.children {
      let needed = self.needed_for(mat, runs);
      let child_runs = child.runs_for(needed);
      let mut child_path = path.to_vec();
      child_path.push(mat);

      child.order_into(needed, child_runs, &child_path, out);
    }

    out.push(BuildJob {
      needed_qty,
      node: self.clone(),
      path: path.to_vec(),
      runs,
      type_id: self.type_id,
    });
  }

  fn raw_into(&self, runs: i64, acc: &mut BTreeMap<i64, i64>) {
    for material in &self.materials {
      let qty = eff_qty(material.base_qty, runs, self.me, self.is_reaction, self.rig_me_factor);
      match self.children.get(&material.type_id) {
        Some(child) => {
          let child_runs = child.runs_for(qty);
          child.raw_into(child_runs, acc);
        }
        None => {
          *acc.entry(material.type_id).or_insert(0) += qty;
        }
      }
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuildPlan {
  pub root: BuildNode,
  /// The only authoritative run count; every sub-build's runs are derived from this and parent demand.
  pub runs: i64,
}

impl BuildPlan {
  pub fn new(root: BuildNode, runs: i64) -> Self {
    BuildPlan {
      root,
      runs: runs.max(1),
    }
  }

  pub fn build_order(&self) -> Vec<BuildJob> {
    let mut out = Vec::new();
    let root_needed = self.runs * self.root.output_per_run;
    self.root.order_into(root_needed, self.runs, &[], &mut out);
    out
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "Foundation for the not-yet-wired build planner UI.")
  )]
  pub fn collect_builds(&self) -> Vec<SubBuild> {
    let mut out = Vec::new();
    self.root.collect_into(self.runs, &[], 0, &mut out);
    out
  }

  /// Collapses [`build_order`] entries that share `(type_id, ME, TE, facility)` into one row at the first
  /// occurrence, summing demand across instances and recomputing `runs = ceil(total_needed / output_per_run)`
  /// — which can be strictly fewer than summing each instance's rounded-up runs. Producer-before-consumer
  /// ordering is preserved; entries of the same type with differing settings stay separate rows.
  pub fn merged_build_order(&self) -> Vec<MergedBuildJob> {
    let order = self.build_order();
    let mut merged: Vec<MergedBuildJob> = Vec::new();
    let mut index: BTreeMap<(i64, i64, i64, Option<i64>), usize> = BTreeMap::new();

    for job in order {
      let key = (job.type_id, job.node.me, job.node.te, job.node.facility);
      // `path` lists type ids from the root's first child down to this job, excluding the root: an empty
      // path is the root product (no consumer), a length-1 path is consumed by the root, and otherwise the
      // consumer is the second-to-last id.
      let parent = match job.path.len() {
        0 => None,
        1 => Some(self.root.type_id),
        len => Some(job.path[len - 2]),
      };
      match index.get(&key) {
        Some(&at) => {
          let row = &mut merged[at];
          row.needed_qty += job.needed_qty;
          if let Some(consumer) = parent {
            if !row.consumers.contains(&consumer) {
              row.consumers.push(consumer);
            }
          } else {
            row.is_root = true;
          }
          row.runs = runs_for(row.needed_qty, row.node.output_per_run);
        }
        None => {
          let mut consumers = Vec::new();
          let mut is_root = false;
          match parent {
            Some(consumer) => consumers.push(consumer),
            None => is_root = true,
          }
          index.insert(key, merged.len());
          merged.push(MergedBuildJob {
            consumers,
            is_root,
            needed_qty: job.needed_qty,
            node: job.node.clone(),
            runs: runs_for(job.needed_qty, job.node.output_per_run),
            type_id: job.type_id,
          });
        }
      }
    }

    merged
  }

  #[cfg_attr(
    not(test),
    expect(dead_code, reason = "Foundation for the not-yet-wired build planner UI.")
  )]
  pub fn needed_blueprints(&self) -> Vec<NeededBlueprint> {
    needed_blueprints_from(&self.merged_build_order())
  }

  pub fn node_count(&self) -> usize {
    self.root.count_nodes()
  }

  pub fn raw_totals(&self) -> Vec<RawTotal> {
    let mut acc = BTreeMap::new();
    self.root.raw_into(self.runs, &mut acc);
    acc
      .into_iter()
      .map(|(type_id, qty)| RawTotal {
        qty,
        type_id,
      })
      .collect()
  }

  pub fn raw_totals_after_stock(&self, allocation: &StockAllocation) -> Vec<RawTotal> {
    self
      .raw_totals()
      .into_iter()
      .filter_map(|total| {
        let net = (total.qty - allocation.drawn_for_type(total.type_id)).max(0);
        (net > 0).then_some(RawTotal {
          qty: net,
          type_id: total.type_id,
        })
      })
      .collect()
  }

  pub fn total_runs_for(&self, type_id: i64) -> i64 {
    self
      .merged_build_order()
      .iter()
      .filter(|row| row.type_id == type_id)
      .map(|row| row.runs)
      .sum()
  }
}

pub trait BuildableLookup<C> {
  fn buildable_inputs(&self, type_id: i64) -> Vec<i64>;

  fn fresh_child(&self, type_id: i64) -> C;

  fn children_of<'a>(&self, child: &'a mut C) -> &'a mut BTreeMap<i64, C>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Material {
  pub base_qty: i64,
  pub type_id: i64,
}

impl Material {
  pub fn new(type_id: i64, base_qty: i64) -> Self {
    Material {
      base_qty,
      type_id,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MergedBuildJob {
  pub consumers: Vec<i64>,
  pub is_root: bool,
  pub needed_qty: i64,
  pub node: BuildNode,
  pub runs: i64,
  pub type_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeededBlueprint {
  pub jobs: i64,
  pub runs: i64,
  pub type_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlanSegment {
  pub clone_id: Option<i64>,
  pub pilot_id: Option<i64>,
  pub runs: i64,
}

impl PlanSegment {
  pub fn unassigned(runs: i64) -> Self {
    PlanSegment {
      clone_id: None,
      pilot_id: None,
      runs,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawTotal {
  pub qty: i64,
  pub type_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigFactors {
  pub fee: f64,
  pub me: f64,
  pub te: f64,
}

impl Default for RigFactors {
  fn default() -> Self {
    RigFactors {
      fee: 1.0,
      me: 1.0,
      te: 1.0,
    }
  }
}

impl RigFactors {
  pub fn from_rigs(rig_type_ids: &[i64], catalog: &HashMap<i64, RigBonus>, security_status: f64) -> Self {
    RigFactors::from_bonuses(derive_rig_bonuses(rig_type_ids, catalog, security_status))
  }

  fn from_bonuses(bonuses: DerivedRigBonuses) -> Self {
    RigFactors {
      fee: 1.0 + bonuses.fee / 100.0,
      me: 1.0 + bonuses.me / 100.0,
      te: 1.0 + bonuses.te / 100.0,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StockAllocation {
  pub drawn_by_pool: HashMap<(i64, i64), i64>,
  pub draws: Vec<StockDraw>,
}

impl StockAllocation {
  pub fn drawn_for_type(&self, type_id: i64) -> i64 {
    self
      .drawn_by_pool
      .iter()
      .filter(|((_, pool_type), _)| *pool_type == type_id)
      .map(|(_, &qty)| qty)
      .sum()
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockDraw {
  pub buy: i64,
  pub drawn: i64,
  pub site: i64,
  pub type_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockSelection {
  pub needed: i64,
  pub site: i64,
  pub type_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubBuild {
  pub depth: usize,
  pub needed_qty: i64,
  pub node: BuildNode,
  pub path: Vec<i64>,
  pub runs: i64,
  pub type_id: i64,
}

/// Allocates on-hand stock to an ORDERED list of use-stock selections, draining each `(site, type_id)` pool in
/// selection order so no physical unit is counted twice across competing jobs that share a pool.
///
/// `on_hand` is the available quantity keyed by `(site, type_id)` (the shape of
/// `store::repo::assets::on_hand_at_build_sites`); the caller passes it in so this stays DB-free and
/// deterministic. Each selection draws `min(remaining pool, needed)`; the rest is left to buy. Returns the
/// per-selection draws (parallel to `selections` by index) plus the total drawn per pool for netting.
pub fn allocate_stock(on_hand: &HashMap<(i64, i64), i64>, selections: &[StockSelection]) -> StockAllocation {
  let mut remaining: HashMap<(i64, i64), i64> = on_hand.clone();
  let mut allocation = StockAllocation::default();

  for selection in selections {
    allocation.draws.push(draw_from_pool(&mut remaining, selection));
  }

  for ((site, type_id), &available) in on_hand {
    let left = remaining.get(&(*site, *type_id)).copied().unwrap_or(0);
    let drawn = available - left;
    if drawn > 0 {
      allocation.drawn_by_pool.insert((*site, *type_id), drawn);
    }
  }

  allocation
}

fn draw_from_pool(remaining: &mut HashMap<(i64, i64), i64>, selection: &StockSelection) -> StockDraw {
  let key = (selection.site, selection.type_id);
  let needed = selection.needed.max(0);
  let pool = remaining.entry(key).or_insert(0);
  let drawn = needed.min(*pool).max(0);
  *pool -= drawn;

  StockDraw {
    buy: needed - drawn,
    drawn,
    site: selection.site,
    type_id: selection.type_id,
  }
}

pub fn merge_segments(stored: &[PlanSegment], total: i64) -> Vec<PlanSegment> {
  let segments = reconcile_segments(stored, total);
  let first = segments[0];

  vec![PlanSegment {
    clone_id: first.clone_id,
    pilot_id: first.pilot_id,
    runs: total,
  }]
}

pub fn reconcile_segments(stored: &[PlanSegment], total: i64) -> Vec<PlanSegment> {
  if stored.is_empty() {
    return vec![PlanSegment::unassigned(total)];
  }

  let mut segments = stored.to_vec();
  let n = segments.len();
  if n == 1 {
    segments[0].runs = total;
    return segments;
  }
  if total < n as i64 {
    let first = segments[0];
    return vec![PlanSegment {
      clone_id: first.clone_id,
      pilot_id: first.pilot_id,
      runs: total,
    }];
  }

  let sum: i64 = segments.iter().map(|segment| segment.runs).sum();
  if sum != total {
    let distribution = distribute_runs(total, n);
    for (segment, runs) in segments.iter_mut().zip(distribution) {
      segment.runs = runs;
    }
  }
  segments
}

pub fn remove_segment(stored: &[PlanSegment], total: i64, index: usize) -> Vec<PlanSegment> {
  let segments = reconcile_segments(stored, total);
  if segments.len() <= 1 || index >= segments.len() {
    return segments;
  }

  let mut next: Vec<PlanSegment> = segments
    .into_iter()
    .enumerate()
    .filter_map(|(i, segment)| (i != index).then_some(segment))
    .collect();
  let distribution = distribute_runs(total, next.len());
  for (segment, runs) in next.iter_mut().zip(distribution) {
    segment.runs = runs;
  }
  next
}

pub fn set_segment_assignment(
  stored: &[PlanSegment],
  total: i64,
  index: usize,
  pilot_id: Option<i64>,
  clone_id: Option<i64>,
) -> Vec<PlanSegment> {
  let mut segments = reconcile_segments(stored, total);
  if let Some(segment) = segments.get_mut(index) {
    segment.clone_id = clone_id;
    segment.pilot_id = pilot_id;
  }
  segments
}

pub fn set_segment_runs(stored: &[PlanSegment], total: i64, index: usize, value: i64) -> Vec<PlanSegment> {
  let mut segments = reconcile_segments(stored, total);
  let n = segments.len();
  if n == 1 || index >= n {
    return segments;
  }

  let clamped = value.clamp(1, total - (n as i64 - 1));
  let distribution = distribute_runs(total - clamped, n - 1);
  let mut others = distribution.into_iter();
  for (i, segment) in segments.iter_mut().enumerate() {
    segment.runs = if i == index {
      clamped
    } else {
      others.next().unwrap_or(1)
    };
  }
  segments
}

pub fn split_segments(stored: &[PlanSegment], total: i64) -> Vec<PlanSegment> {
  let mut segments = reconcile_segments(stored, total);
  if total < segments.len() as i64 + 1 {
    return segments;
  }

  let mut largest = 0;
  for (i, segment) in segments.iter().enumerate() {
    if segment.runs > segments[largest].runs {
      largest = i;
    }
  }
  let head = (segments[largest].runs + 1) / 2;
  let tail = segments[largest].runs - head;
  if tail < 1 {
    return segments;
  }
  segments[largest].runs = head;
  segments.insert(largest + 1, PlanSegment::unassigned(tail));
  segments
}

fn distribute_runs(total: i64, k: usize) -> Vec<i64> {
  if k == 0 {
    return Vec::new();
  }

  let k = k as i64;
  let base = total / k;
  let remainder = total - base * k;
  (0..k).map(|i| base + i64::from(i < remainder)).collect()
}

pub fn eff_qty(base_qty: i64, runs: i64, me: i64, is_reaction: bool, rig_me_factor: f64) -> i64 {
  if is_reaction {
    return base_qty * runs;
  }

  let reduced = ((base_qty as f64) * (1.0 - (me as f64) / 100.0) * rig_me_factor).ceil() as i64;
  reduced.max(1) * runs
}

pub fn needed_blueprints_from(merged: &[MergedBuildJob]) -> Vec<NeededBlueprint> {
  let mut acc: BTreeMap<i64, NeededBlueprint> = BTreeMap::new();
  for row in merged {
    let entry = acc.entry(row.type_id).or_insert(NeededBlueprint {
      jobs: 0,
      runs: 0,
      type_id: row.type_id,
    });
    entry.jobs += 1;
    entry.runs += row.runs;
  }
  acc.into_values().collect()
}

pub fn runs_for(needed_qty: i64, output_per_run: i64) -> i64 {
  let per_run = output_per_run.max(1);
  let demand = needed_qty.max(0);
  ((demand + per_run - 1) / per_run).max(1)
}

#[cfg_attr(
  not(test),
  expect(dead_code, reason = "Foundation for the not-yet-wired build planner UI.")
)]
pub fn expand_to_raw<C, L>(children: &mut BTreeMap<i64, C>, type_id: i64, lookup: &L)
where
  L: BuildableLookup<C>,
{
  expand_to_raw_into(children, type_id, lookup);
}

fn expand_to_raw_into<C, L>(children: &mut BTreeMap<i64, C>, type_id: i64, lookup: &L)
where
  L: BuildableLookup<C>,
{
  for mat in lookup.buildable_inputs(type_id) {
    let child = children.entry(mat).or_insert_with(|| lookup.fresh_child(mat));
    let grandchildren = lookup.children_of(child);
    expand_to_raw_into(grandchildren, mat, lookup);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const FUEL_BLOCK: i64 = 4051;

  const GAS: i64 = 16634;

  const HULK: i64 = 22544;

  const PYERITE: i64 = 35;

  const RETRIEVER: i64 = 17478;

  const TRITANIUM: i64 = 34;

  fn hulk_plan() -> BuildPlan {
    let mut root = BuildNode::new(
      HULK,
      1,
      false,
      vec![Material::new(RETRIEVER, 10), Material::new(TRITANIUM, 5)],
    );
    let retriever = BuildNode::new(RETRIEVER, 1, false, vec![Material::new(TRITANIUM, 1)]);
    root.children.insert(RETRIEVER, retriever);

    BuildPlan::new(root, 1)
  }

  mod allocate_stock {
    use pretty_assertions::assert_eq;

    use super::*;

    const COMPONENT_A: i64 = 700;

    const SITE_A: i64 = 60_003_760;

    const SITE_B: i64 = 60_008_494;

    const TRITANIUM: i64 = 34;

    #[test]
    fn it_caps_the_draw_at_the_demand_when_stock_oversupplies() {
      let on_hand = HashMap::from([((SITE_A, TRITANIUM), 1000)]);
      let selections = [StockSelection {
        needed: 300,
        site: SITE_A,
        type_id: TRITANIUM,
      }];

      let allocation = allocate_stock(&on_hand, &selections);

      assert_eq!(allocation.draws[0].drawn, 300);
      assert_eq!(allocation.draws[0].buy, 0);
      assert_eq!(allocation.drawn_for_type(TRITANIUM), 300);
    }

    #[test]
    fn it_drains_a_shared_pool_in_selection_order() {
      let on_hand = HashMap::from([((SITE_A, TRITANIUM), 1000)]);
      let selections = [
        StockSelection {
          needed: 2000,
          site: SITE_A,
          type_id: TRITANIUM,
        },
        StockSelection {
          needed: 1000,
          site: SITE_A,
          type_id: TRITANIUM,
        },
      ];

      let allocation = allocate_stock(&on_hand, &selections);

      assert_eq!(allocation.draws[0].drawn, 1000);
      assert_eq!(allocation.draws[0].buy, 1000);
      assert_eq!(allocation.draws[1].drawn, 0);
      assert_eq!(allocation.draws[1].buy, 1000);
      assert_eq!(allocation.drawn_for_type(TRITANIUM), 1000);
    }

    #[test]
    fn it_keys_pools_separately_per_site() {
      let on_hand = HashMap::from([((SITE_A, TRITANIUM), 100), ((SITE_B, TRITANIUM), 40)]);
      let selections = [
        StockSelection {
          needed: 500,
          site: SITE_A,
          type_id: TRITANIUM,
        },
        StockSelection {
          needed: 500,
          site: SITE_B,
          type_id: TRITANIUM,
        },
      ];

      let allocation = allocate_stock(&on_hand, &selections);

      assert_eq!(allocation.draws[0].drawn, 100);
      assert_eq!(allocation.draws[1].drawn, 40);
      assert_eq!(allocation.drawn_by_pool.get(&(SITE_A, TRITANIUM)).copied(), Some(100));
      assert_eq!(allocation.drawn_by_pool.get(&(SITE_B, TRITANIUM)).copied(), Some(40));
      assert_eq!(allocation.drawn_for_type(TRITANIUM), 140);
    }

    #[test]
    fn it_leaves_an_empty_on_hand_map_untouched() {
      let selections = [StockSelection {
        needed: 100,
        site: SITE_A,
        type_id: TRITANIUM,
      }];

      let allocation = allocate_stock(&HashMap::new(), &selections);

      assert_eq!(allocation.draws[0].drawn, 0);
      assert_eq!(allocation.draws[0].buy, 100);
      assert!(allocation.drawn_by_pool.is_empty());
    }

    #[test]
    fn it_leaves_the_uncovered_remainder_to_break_down() {
      let on_hand = HashMap::from([((SITE_A, COMPONENT_A), 5)]);
      let selections = [StockSelection {
        needed: 10,
        site: SITE_A,
        type_id: COMPONENT_A,
      }];

      let allocation = allocate_stock(&on_hand, &selections);

      assert_eq!(allocation.draws[0].drawn, 5);
      assert_eq!(allocation.draws[0].buy, 5);
    }
  }

  mod build_node {
    use super::*;

    mod count_nodes {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_counts_a_leaf_as_one() {
        let leaf = BuildNode::new(TRITANIUM, 1, false, vec![]);

        assert_eq!(leaf.count_nodes(), 1);
      }

      #[test]
      fn it_counts_every_node_in_the_tree() {
        let plan = hulk_plan();

        assert_eq!(plan.root.count_nodes(), 2);
        assert_eq!(plan.node_count(), 2);
      }
    }
  }

  mod build_plan {
    use super::*;

    mod build_order {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_orders_deeper_dependencies_first() {
        let tritanium = BuildNode::new(TRITANIUM, 1, false, vec![Material::new(PYERITE, 2)]);
        let mut retriever = BuildNode::new(RETRIEVER, 1, false, vec![Material::new(TRITANIUM, 4)]);
        retriever.children.insert(TRITANIUM, tritanium);
        let mut root = BuildNode::new(HULK, 1, false, vec![Material::new(RETRIEVER, 2)]);
        root.children.insert(RETRIEVER, retriever);

        let order = BuildPlan::new(root, 1).build_order();

        let ids: Vec<i64> = order.iter().map(|job| job.type_id).collect();
        assert_eq!(ids, vec![TRITANIUM, RETRIEVER, HULK]);
      }

      #[test]
      fn it_sequences_dependencies_before_the_final_product() {
        let plan = hulk_plan();

        let order = plan.build_order();

        let ids: Vec<i64> = order.iter().map(|job| job.type_id).collect();
        assert_eq!(ids, vec![RETRIEVER, HULK]);

        assert_eq!(order.last().unwrap().path, Vec::<i64>::new());
        assert_eq!(order.last().unwrap().runs, 1);
      }

      #[test]
      fn it_threads_the_needed_quantity_onto_each_job() {
        let plan = hulk_plan();

        let order = plan.build_order();

        let retriever = order.iter().find(|job| job.type_id == RETRIEVER).unwrap();
        let hulk = order.iter().find(|job| job.type_id == HULK).unwrap();
        assert_eq!(retriever.needed_qty, 10);
        assert_eq!(hulk.needed_qty, 1);
      }
    }

    mod collect_builds {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_locks_sub_build_runs_to_parent_demand() {
        let plan = hulk_plan();

        let builds = plan.collect_builds();

        assert_eq!(builds.len(), 1);
        assert_eq!(builds[0].type_id, RETRIEVER);
        assert_eq!(builds[0].needed_qty, 10);
        assert_eq!(builds[0].runs, 10);
        assert_eq!(builds[0].depth, 1);
        assert_eq!(builds[0].path, vec![RETRIEVER]);
      }

      #[test]
      fn it_scales_locked_runs_with_root_runs_and_me() {
        let mut plan = hulk_plan();
        plan.runs = 2;
        plan.root.me = 10;

        let builds = plan.collect_builds();

        assert_eq!(builds[0].needed_qty, 18);
        assert_eq!(builds[0].runs, 18);
      }
    }

    mod merged_build_order {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_keeps_divergent_facility_entries_separate() {
        const WIDGET: i64 = 900;
        const GADGET: i64 = 901;
        const COG: i64 = 902;
        let mut cog_a = BuildNode::new(COG, 1, false, vec![]);
        cog_a.facility = Some(1);
        let mut cog_b = BuildNode::new(COG, 1, false, vec![]);
        cog_b.facility = Some(2);
        let mut widget = BuildNode::new(WIDGET, 1, false, vec![Material::new(COG, 3)]);
        let mut gadget = BuildNode::new(GADGET, 1, false, vec![Material::new(COG, 2)]);
        widget.children.insert(COG, cog_a);
        gadget.children.insert(COG, cog_b);
        let mut root = BuildNode::new(HULK, 1, false, vec![Material::new(WIDGET, 1), Material::new(GADGET, 1)]);
        root.children.insert(WIDGET, widget);
        root.children.insert(GADGET, gadget);

        let merged = BuildPlan::new(root, 1).merged_build_order();

        let cog_rows: Vec<&MergedBuildJob> = merged.iter().filter(|row| row.type_id == COG).collect();
        assert_eq!(cog_rows.len(), 2);
      }

      #[test]
      fn it_marks_the_root_product_row() {
        let plan = hulk_plan();

        let merged = plan.merged_build_order();

        let hulk_row = merged.iter().find(|row| row.type_id == HULK).unwrap();
        assert!(hulk_row.is_root);
        assert!(hulk_row.consumers.is_empty());

        let retriever_row = merged.iter().find(|row| row.type_id == RETRIEVER).unwrap();
        assert!(!retriever_row.is_root);
        assert_eq!(retriever_row.consumers, vec![HULK]);
      }

      #[test]
      fn it_merges_same_setting_entries_and_recomputes_runs() {
        const WIDGET: i64 = 900;
        const GADGET: i64 = 901;
        const COG: i64 = 902;
        let cog = BuildNode::new(COG, 1, false, vec![]);
        let mut widget = BuildNode::new(WIDGET, 1, false, vec![Material::new(COG, 3)]);
        let mut gadget = BuildNode::new(GADGET, 1, false, vec![Material::new(COG, 2)]);
        widget.children.insert(COG, cog.clone());
        gadget.children.insert(COG, cog);
        let mut root = BuildNode::new(HULK, 1, false, vec![Material::new(WIDGET, 1), Material::new(GADGET, 1)]);
        root.children.insert(WIDGET, widget);
        root.children.insert(GADGET, gadget);

        let merged = BuildPlan::new(root, 1).merged_build_order();

        let cog_row = merged.iter().find(|row| row.type_id == COG).unwrap();
        assert_eq!(cog_row.needed_qty, 5);
        assert_eq!(cog_row.runs, 5);

        let mut consumers = cog_row.consumers.clone();
        consumers.sort();
        assert_eq!(consumers, vec![WIDGET, GADGET]);
      }

      #[test]
      fn it_preserves_producer_before_consumer_ordering() {
        const WIDGET: i64 = 900;
        const GADGET: i64 = 901;
        const COG: i64 = 902;
        let cog = BuildNode::new(COG, 1, false, vec![]);
        let mut widget = BuildNode::new(WIDGET, 1, false, vec![Material::new(COG, 3)]);
        let mut gadget = BuildNode::new(GADGET, 1, false, vec![Material::new(COG, 2)]);
        widget.children.insert(COG, cog.clone());
        gadget.children.insert(COG, cog);
        let mut root = BuildNode::new(HULK, 1, false, vec![Material::new(WIDGET, 1), Material::new(GADGET, 1)]);
        root.children.insert(WIDGET, widget);
        root.children.insert(GADGET, gadget);

        let merged = BuildPlan::new(root, 1).merged_build_order();

        let positions: BTreeMap<i64, usize> = merged.iter().enumerate().map(|(i, row)| (row.type_id, i)).collect();
        assert!(positions[&COG] < positions[&WIDGET]);
        assert!(positions[&COG] < positions[&GADGET]);
        assert!(positions[&WIDGET] < positions[&HULK]);
        assert!(positions[&GADGET] < positions[&HULK]);
      }

      #[test]
      fn it_uses_ceil_of_summed_demand_not_sum_of_ceils() {
        const WIDGET: i64 = 900;
        const GADGET: i64 = 901;
        const COG: i64 = 902;
        let cog = BuildNode::new(COG, 2, false, vec![]);
        let mut widget = BuildNode::new(WIDGET, 1, false, vec![Material::new(COG, 3)]);
        let mut gadget = BuildNode::new(GADGET, 1, false, vec![Material::new(COG, 3)]);
        widget.children.insert(COG, cog.clone());
        gadget.children.insert(COG, cog);
        let mut root = BuildNode::new(HULK, 1, false, vec![Material::new(WIDGET, 1), Material::new(GADGET, 1)]);
        root.children.insert(WIDGET, widget);
        root.children.insert(GADGET, gadget);

        let merged = BuildPlan::new(root, 1).merged_build_order();

        let cog_row = merged.iter().find(|row| row.type_id == COG).unwrap();
        assert_eq!(cog_row.needed_qty, 6);
        assert_eq!(cog_row.runs, 3);
      }
    }

    mod needed_blueprints {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_aggregates_the_merged_order_by_type() {
        const WIDGET: i64 = 900;
        const GADGET: i64 = 901;
        const COG: i64 = 902;
        let cog = BuildNode::new(COG, 1, false, vec![]);
        let mut widget = BuildNode::new(WIDGET, 1, false, vec![Material::new(COG, 3)]);
        let mut gadget = BuildNode::new(GADGET, 1, false, vec![Material::new(COG, 2)]);
        widget.children.insert(COG, cog.clone());
        gadget.children.insert(COG, cog);
        let mut root = BuildNode::new(HULK, 1, false, vec![Material::new(WIDGET, 1), Material::new(GADGET, 1)]);
        root.children.insert(WIDGET, widget);
        root.children.insert(GADGET, gadget);

        let plan = BuildPlan::new(root, 1);
        let blueprints = plan.needed_blueprints();

        let cog_bp = blueprints.iter().find(|bp| bp.type_id == COG).unwrap();
        assert_eq!(cog_bp.jobs, 1);
        assert_eq!(cog_bp.runs, 5);

        let total_runs: i64 = plan.merged_build_order().iter().map(|row| row.runs).sum();
        assert_eq!(blueprints.iter().map(|bp| bp.runs).sum::<i64>(), total_runs);
        assert_eq!(
          blueprints.iter().map(|bp| bp.jobs).sum::<i64>(),
          plan.merged_build_order().len() as i64
        );
      }
    }

    mod raw_totals {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_ignores_me_when_rolling_up_a_reaction_node() {
        let mut root = BuildNode::new(FUEL_BLOCK, 40, true, vec![Material::new(GAS, 25)]);
        root.me = 10;

        let totals = BuildPlan::new(root, 2).raw_totals();

        assert_eq!(
          totals,
          vec![RawTotal {
            qty: 50,
            type_id: GAS,
          }]
        );
      }

      #[test]
      fn it_keeps_a_material_raw_when_it_is_not_broken_down() {
        let root = BuildNode::new(
          HULK,
          1,
          false,
          vec![Material::new(RETRIEVER, 10), Material::new(TRITANIUM, 5)],
        );

        let totals = BuildPlan::new(root, 1).raw_totals();

        assert_eq!(
          totals,
          vec![
            RawTotal {
              qty: 5,
              type_id: TRITANIUM,
            },
            RawTotal {
              qty: 10,
              type_id: RETRIEVER,
            },
          ]
        );
      }

      #[test]
      fn it_rolls_up_multi_level_breakdowns_to_raw_inputs() {
        let plan = hulk_plan();

        let totals = plan.raw_totals();

        assert_eq!(
          totals,
          vec![RawTotal {
            qty: 15,
            type_id: TRITANIUM,
          }]
        );
      }
    }

    mod raw_totals_after_stock {
      use pretty_assertions::assert_eq;

      use super::*;

      const SITE: i64 = 60_003_760;

      #[test]
      fn it_drops_a_type_fully_covered_by_stock() {
        let plan = hulk_plan();
        let on_hand = HashMap::from([((SITE, TRITANIUM), 100)]);
        let selections = [StockSelection {
          needed: 15,
          site: SITE,
          type_id: TRITANIUM,
        }];

        let allocation = allocate_stock(&on_hand, &selections);
        let netted = plan.raw_totals_after_stock(&allocation);

        assert!(netted.is_empty());
      }

      #[test]
      fn it_is_unchanged_with_an_empty_allocation() {
        let plan = hulk_plan();

        let netted = plan.raw_totals_after_stock(&StockAllocation::default());

        assert_eq!(netted, plan.raw_totals());
      }

      #[test]
      fn it_subtracts_drawn_stock_from_a_types_demand() {
        let plan = hulk_plan();
        let on_hand = HashMap::from([((SITE, TRITANIUM), 6)]);
        let selections = [StockSelection {
          needed: 6,
          site: SITE,
          type_id: TRITANIUM,
        }];

        let allocation = allocate_stock(&on_hand, &selections);
        let netted = plan.raw_totals_after_stock(&allocation);

        assert_eq!(
          netted,
          vec![RawTotal {
            qty: 9,
            type_id: TRITANIUM,
          }]
        );
      }
    }

    mod total_runs_for {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_zero_for_a_type_not_in_the_order() {
        let plan = hulk_plan();

        assert_eq!(plan.total_runs_for(999), 0);
      }

      #[test]
      fn it_sums_runs_across_merged_rows_of_a_type() {
        const WIDGET: i64 = 900;
        const GADGET: i64 = 901;
        const COG: i64 = 902;
        let cog = BuildNode::new(COG, 1, false, vec![]);
        let mut widget = BuildNode::new(WIDGET, 1, false, vec![Material::new(COG, 3)]);
        let mut gadget = BuildNode::new(GADGET, 1, false, vec![Material::new(COG, 3)]);
        widget.children.insert(COG, cog.clone());
        gadget.children.insert(COG, cog);
        let mut root = BuildNode::new(HULK, 1, false, vec![Material::new(WIDGET, 1), Material::new(GADGET, 1)]);
        root.children.insert(WIDGET, widget);
        root.children.insert(GADGET, gadget);

        let plan = BuildPlan::new(root, 1);

        assert_eq!(plan.total_runs_for(COG), 6);
      }
    }
  }

  mod distribute_runs {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_always_sums_to_the_total() {
      let distribution = distribute_runs(17, 5);

      assert_eq!(distribution.iter().sum::<i64>(), 17);
    }

    #[test]
    fn it_front_loads_the_remainder() {
      let distribution = distribute_runs(17, 5);

      assert_eq!(distribution, vec![4, 4, 3, 3, 3]);
    }

    #[test]
    fn it_returns_empty_for_zero_buckets() {
      let distribution = distribute_runs(10, 0);

      assert!(distribution.is_empty());
    }

    #[test]
    fn it_splits_evenly_when_divisible() {
      let distribution = distribute_runs(12, 4);

      assert_eq!(distribution, vec![3, 3, 3, 3]);
    }
  }

  mod eff_qty {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ceils_the_me_reduced_quantity() {
      let result = eff_qty(7, 1, 10, false, 1.0);

      assert_eq!(result, 7);
    }

    #[test]
    fn it_floors_a_reduced_quantity_at_one_per_run() {
      let result = eff_qty(1, 4, 10, false, 1.0);

      assert_eq!(result, 4);
    }

    #[test]
    fn it_ignores_me_for_reactions() {
      let with_me = eff_qty(100, 3, 10, true, 1.0);
      let without_me = eff_qty(100, 3, 0, true, 1.0);

      assert_eq!(with_me, 300);
      assert_eq!(without_me, 300);
    }

    #[test]
    fn it_reduces_manufacturing_quantity_by_me_then_scales_by_runs() {
      let result = eff_qty(100, 3, 10, false, 1.0);

      assert_eq!(result, 270);
    }

    #[test]
    fn it_reduces_quantity_by_the_rig_me_factor() {
      let result = eff_qty(100, 1, 0, false, 0.98);

      assert_eq!(result, 98);
    }

    #[test]
    fn it_stacks_blueprint_me_and_rig_me_before_rounding_once() {
      let result = eff_qty(100, 1, 10, false, 0.96);

      assert_eq!(result, 87);
    }

    #[test]
    fn it_ignores_the_rig_me_factor_for_reactions() {
      let result = eff_qty(100, 3, 0, true, 0.5);

      assert_eq!(result, 300);
    }
  }

  mod rig_factors {
    use pretty_assertions::assert_eq;

    use super::*;

    fn catalog() -> HashMap<i64, RigBonus> {
      HashMap::from([(
        100,
        RigBonus {
          fee: -10.0,
          me: -2.0,
          te: -20.0,
        },
      )])
    }

    #[test]
    fn it_defaults_to_neutral_factors() {
      assert_eq!(
        RigFactors::default(),
        RigFactors {
          fee: 1.0,
          me: 1.0,
          te: 1.0
        }
      );
    }

    #[test]
    fn it_converts_hi_sec_bonuses_into_multiplicative_factors() {
      let factors = RigFactors::from_rigs(&[100], &catalog(), 0.9);

      assert!((factors.fee - 0.9).abs() < 1e-9);
      assert!((factors.me - 0.98).abs() < 1e-9);
      assert!((factors.te - 0.8).abs() < 1e-9);
    }

    #[test]
    fn it_scales_bonuses_by_the_low_sec_band() {
      let factors = RigFactors::from_rigs(&[100], &catalog(), 0.4);

      assert!((factors.me - (1.0 + -2.0 * 1.9 / 100.0)).abs() < 1e-9);
      assert!((factors.te - (1.0 + -20.0 * 1.9 / 100.0)).abs() < 1e-9);
      assert!((factors.fee - (1.0 + -10.0 * 1.9 / 100.0)).abs() < 1e-9);
    }

    #[test]
    fn it_scales_bonuses_by_the_null_sec_band() {
      let factors = RigFactors::from_rigs(&[100], &catalog(), -1.0);

      assert!((factors.me - (1.0 + -2.0 * 2.1 / 100.0)).abs() < 1e-9);
    }

    #[test]
    fn it_returns_neutral_factors_for_an_empty_rig_list() {
      let factors = RigFactors::from_rigs(&[], &catalog(), 0.9);

      assert_eq!(factors, RigFactors::default());
    }
  }

  mod expand_to_raw {
    use std::collections::BTreeMap;

    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(Clone, Debug, Default, PartialEq)]
    struct TestNode {
      children: BTreeMap<i64, TestNode>,
    }

    struct Bom {
      inputs: BTreeMap<i64, Vec<i64>>,
    }

    impl BuildableLookup<TestNode> for Bom {
      fn buildable_inputs(&self, type_id: i64) -> Vec<i64> {
        self.inputs.get(&type_id).cloned().unwrap_or_default()
      }

      fn fresh_child(&self, _type_id: i64) -> TestNode {
        TestNode::default()
      }

      fn children_of<'a>(&self, child: &'a mut TestNode) -> &'a mut BTreeMap<i64, TestNode> {
        &mut child.children
      }
    }

    fn built_ids(children: &BTreeMap<i64, TestNode>, out: &mut Vec<i64>) {
      for (&id, node) in children {
        out.push(id);
        built_ids(&node.children, out);
      }
    }

    #[test]
    fn it_breaks_down_a_multi_level_tree_to_raw_inputs() {
      let bom = Bom {
        inputs: BTreeMap::from([(HULK, vec![RETRIEVER]), (RETRIEVER, vec![])]),
      };
      let mut children = BTreeMap::new();

      expand_to_raw(&mut children, HULK, &bom);

      let mut ids = Vec::new();
      built_ids(&children, &mut ids);
      assert_eq!(ids, vec![RETRIEVER]);
      assert!(children[&RETRIEVER].children.is_empty());
    }

    #[test]
    fn it_descends_through_a_buildable_intermediate() {
      const WIDGET: i64 = 900;
      const GADGET: i64 = 901;
      const COG: i64 = 902;
      let bom = Bom {
        inputs: BTreeMap::from([(WIDGET, vec![GADGET]), (GADGET, vec![COG]), (COG, vec![])]),
      };
      let mut children = BTreeMap::new();

      expand_to_raw(&mut children, WIDGET, &bom);

      let mut ids = Vec::new();
      built_ids(&children, &mut ids);
      assert_eq!(ids, vec![GADGET, COG]);
    }

    #[test]
    fn it_expands_a_reaction_input() {
      const FUEL: i64 = 4051;
      const COMPOSITE: i64 = 16_670;
      let bom = Bom {
        inputs: BTreeMap::from([(FUEL, vec![COMPOSITE]), (COMPOSITE, vec![])]),
      };
      let mut children = BTreeMap::new();

      expand_to_raw(&mut children, FUEL, &bom);

      let mut ids = Vec::new();
      built_ids(&children, &mut ids);
      assert_eq!(ids, vec![COMPOSITE]);
    }

    #[test]
    fn it_keeps_and_deepens_an_existing_partial_breakdown() {
      const WIDGET: i64 = 900;
      const GADGET: i64 = 901;
      const COG: i64 = 902;
      let bom = Bom {
        inputs: BTreeMap::from([(WIDGET, vec![GADGET]), (GADGET, vec![COG]), (COG, vec![])]),
      };
      let mut children = BTreeMap::from([(GADGET, TestNode::default())]);

      expand_to_raw(&mut children, WIDGET, &bom);

      assert!(children[&GADGET].children.contains_key(&COG));
    }
  }

  mod merge_segments {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collapses_to_one_segment_with_the_full_total() {
      let stored = vec![
        PlanSegment {
          clone_id: Some(7),
          pilot_id: Some(1),
          runs: 4,
        },
        PlanSegment::unassigned(6),
      ];

      let merged = merge_segments(&stored, 10);

      assert_eq!(merged.len(), 1);
      assert_eq!(merged[0].runs, 10);
    }

    #[test]
    fn it_keeps_the_first_segments_assignment() {
      let stored = vec![
        PlanSegment {
          clone_id: Some(7),
          pilot_id: Some(1),
          runs: 4,
        },
        PlanSegment {
          clone_id: Some(9),
          pilot_id: Some(2),
          runs: 6,
        },
      ];

      let merged = merge_segments(&stored, 10);

      assert_eq!(merged[0].pilot_id, Some(1));
      assert_eq!(merged[0].clone_id, Some(7));
    }
  }

  mod reconcile_segments {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collapses_to_one_segment_when_the_total_drops_below_the_count() {
      let stored = vec![
        PlanSegment {
          clone_id: Some(7),
          pilot_id: Some(1),
          runs: 2,
        },
        PlanSegment::unassigned(2),
        PlanSegment::unassigned(2),
      ];

      let reconciled = reconcile_segments(&stored, 1);

      assert_eq!(reconciled.len(), 1);
      assert_eq!(reconciled[0].runs, 1);
      assert_eq!(reconciled[0].pilot_id, Some(1));
    }

    #[test]
    fn it_keeps_assignments_while_rebalancing_runs() {
      let stored = vec![
        PlanSegment {
          clone_id: Some(7),
          pilot_id: Some(1),
          runs: 5,
        },
        PlanSegment {
          clone_id: Some(9),
          pilot_id: Some(2),
          runs: 5,
        },
      ];

      let reconciled = reconcile_segments(&stored, 7);

      assert_eq!(reconciled.iter().map(|s| s.runs).collect::<Vec<_>>(), vec![4, 3]);
      assert_eq!(reconciled[0].pilot_id, Some(1));
      assert_eq!(reconciled[1].pilot_id, Some(2));
    }

    #[test]
    fn it_returns_one_full_unassigned_segment_when_absent() {
      let reconciled = reconcile_segments(&[], 12);

      assert_eq!(reconciled, vec![PlanSegment::unassigned(12)]);
    }

    #[test]
    fn it_sums_to_the_total_after_any_change() {
      let stored = vec![
        PlanSegment::unassigned(3),
        PlanSegment::unassigned(3),
        PlanSegment::unassigned(3),
      ];

      let reconciled = reconcile_segments(&stored, 100);

      assert_eq!(reconciled.iter().map(|s| s.runs).sum::<i64>(), 100);
    }

    #[test]
    fn it_tracks_the_total_for_a_single_segment() {
      let stored = vec![PlanSegment {
        clone_id: Some(7),
        pilot_id: Some(1),
        runs: 3,
      }];

      let reconciled = reconcile_segments(&stored, 25);

      assert_eq!(reconciled.len(), 1);
      assert_eq!(reconciled[0].runs, 25);
      assert_eq!(reconciled[0].pilot_id, Some(1));
    }
  }

  mod remove_segment {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_folds_runs_back_into_the_survivors() {
      let stored = vec![
        PlanSegment::unassigned(4),
        PlanSegment::unassigned(3),
        PlanSegment::unassigned(3),
      ];

      let next = remove_segment(&stored, 10, 2);

      assert_eq!(next.len(), 2);
      assert_eq!(next.iter().map(|s| s.runs).sum::<i64>(), 10);
    }

    #[test]
    fn it_keeps_the_last_segment() {
      let stored = vec![PlanSegment::unassigned(10)];

      let next = remove_segment(&stored, 10, 0);

      assert_eq!(next.len(), 1);
    }
  }

  mod runs_for {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_at_least_one_run() {
      let result = runs_for(0, 5);

      assert_eq!(result, 1);
    }

    #[test]
    fn it_rounds_runs_up_to_cover_demand() {
      let result = runs_for(11, 4);

      assert_eq!(result, 3);
    }
  }

  mod set_segment_assignment {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_assigns_a_pilot_and_clone_to_an_absent_segment() {
      let next = set_segment_assignment(&[], 10, 0, Some(7), Some(3));

      assert_eq!(next.len(), 1);
      assert_eq!(next[0].pilot_id, Some(7));
      assert_eq!(next[0].clone_id, Some(3));
      assert_eq!(next[0].runs, 10);
    }

    #[test]
    fn it_clears_an_assignment_when_both_ids_are_none() {
      let stored = vec![PlanSegment {
        clone_id: Some(3),
        pilot_id: Some(7),
        runs: 10,
      }];

      let next = set_segment_assignment(&stored, 10, 0, None, None);

      assert_eq!(next[0].pilot_id, None);
      assert_eq!(next[0].clone_id, None);
    }

    #[test]
    fn it_leaves_other_segments_untouched() {
      let stored = vec![PlanSegment::unassigned(5), PlanSegment::unassigned(5)];

      let next = set_segment_assignment(&stored, 10, 1, Some(2), None);

      assert_eq!(next[0].pilot_id, None);
      assert_eq!(next[1].pilot_id, Some(2));
      assert_eq!(next[1].clone_id, None);
    }

    #[test]
    fn it_ignores_an_out_of_range_index() {
      let next = set_segment_assignment(&[], 10, 9, Some(7), Some(3));

      assert_eq!(next[0].pilot_id, None);
    }
  }

  mod set_segment_runs {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_so_every_other_segment_keeps_a_run() {
      let stored = vec![
        PlanSegment::unassigned(5),
        PlanSegment::unassigned(3),
        PlanSegment::unassigned(2),
      ];

      let next = set_segment_runs(&stored, 10, 0, 100);

      assert_eq!(next[0].runs, 8);
      assert_eq!(next.iter().map(|s| s.runs).sum::<i64>(), 10);
    }

    #[test]
    fn it_leaves_a_single_segment_unchanged() {
      let stored = vec![PlanSegment::unassigned(10)];

      let next = set_segment_runs(&stored, 10, 0, 3);

      assert_eq!(next[0].runs, 10);
    }

    #[test]
    fn it_sets_the_target_and_spreads_the_remainder() {
      let stored = vec![
        PlanSegment::unassigned(3),
        PlanSegment::unassigned(3),
        PlanSegment::unassigned(4),
      ];

      let next = set_segment_runs(&stored, 10, 1, 6);

      assert_eq!(next[1].runs, 6);
      assert_eq!(next.iter().map(|s| s.runs).sum::<i64>(), 10);
    }
  }

  mod split_segments {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_halves_the_largest_segment() {
      let stored = vec![PlanSegment::unassigned(10)];

      let next = split_segments(&stored, 10);

      assert_eq!(next.len(), 2);
      assert_eq!(next.iter().map(|s| s.runs).collect::<Vec<_>>(), vec![5, 5]);
    }

    #[test]
    fn it_keeps_the_total_after_splitting() {
      let stored = vec![PlanSegment::unassigned(7)];

      let next = split_segments(&stored, 7);

      assert_eq!(next.iter().map(|s| s.runs).sum::<i64>(), 7);
    }

    #[test]
    fn it_refuses_to_split_when_runs_are_too_few() {
      let stored = vec![PlanSegment::unassigned(1), PlanSegment::unassigned(1)];

      let next = split_segments(&stored, 2);

      assert_eq!(next.len(), 2);
    }
  }
}
