use std::collections::{BTreeMap, HashMap};

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
  /// Keyed by the material type_id that is built in-house rather than consumed raw; sub-build runs are derived
  /// from parent demand, so a child's own `output_per_run` is authoritative but its run count is not.
  pub children: BTreeMap<i64, BuildNode>,
  pub facility: Option<i64>,
  /// The picked install structure/station id (the build site) for this type, when one was chosen. Keys a
  /// per-site material pool in a later allocation pass; `facility` carries the same site's solar system.
  pub facility_structure: Option<i64>,
  pub is_reaction: bool,
  pub materials: Vec<Material>,
  pub me: i64,
  pub output_per_run: i64,
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
      .map(|m| eff_qty(m.base_qty, runs, self.me, self.is_reaction))
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
      let qty = eff_qty(material.base_qty, runs, self.me, self.is_reaction);
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

  /// [`raw_totals`](Self::raw_totals) with reserved on-hand stock netted out: each raw type's demand is reduced
  /// by the stock `allocation` drew for it (capped at the demand, never negative), and a type fully covered by
  /// stock drops off the buy list entirely. With an empty allocation this equals [`raw_totals`](Self::raw_totals).
  ///
  /// Allocation is computed against the buildable demand the caller surfaced, so a breakdown on a partially
  /// covered job naturally applies only to the uncovered remainder: the breakdown deepens the build tree, the
  /// netting subtracts the same drawn stock from whatever raw inputs that remainder rolls up to.
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
  /// Distinct parent type ids that consume this row's output (empty when only the root product does).
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
pub struct RawTotal {
  pub qty: i64,
  pub type_id: i64,
}

/// The outcome of a [`allocate_stock`] pass: one [`StockDraw`] per input selection (parallel by index) plus
/// the total drawn per `(site, type_id)` pool, used to net reserved stock through [`BuildPlan::raw_totals`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StockAllocation {
  pub draws: Vec<StockDraw>,
  pub drawn_by_pool: HashMap<(i64, i64), i64>,
}

impl StockAllocation {
  /// Total stock drawn for `type_id` across every site, the amount [`BuildPlan::raw_totals_after_stock`]
  /// subtracts from that type's raw demand.
  pub fn drawn_for_type(&self, type_id: i64) -> i64 {
    self
      .drawn_by_pool
      .iter()
      .filter(|((_, pool_type), _)| *pool_type == type_id)
      .map(|(_, &qty)| qty)
      .sum()
  }
}

/// How much one [`StockSelection`] drew from on-hand stock and how much it must still buy. `buy` is the
/// uncovered remainder a breakdown then applies to (only the part stock did not cover).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockDraw {
  pub buy: i64,
  pub drawn: i64,
  pub site: i64,
  pub type_id: i64,
}

/// One job the user opted to draw from on-hand stock: `needed` units of `type_id` wanted at `site`. Several
/// selections can name the same `(site, type_id)` pool; [`allocate_stock`] drains it in selection order so no
/// physical unit is counted twice.
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

/// Draws one selection's demand from its `(site, type_id)` pool, decrementing `remaining` so a later
/// selection on the same pool sees only what is left.
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

/// Effective material need: `ceil(base * (1 - me/100))` floored at 1 per run, then scaled by runs. Reactions
/// bypass ME entirely (`base * runs`).
pub fn eff_qty(base_qty: i64, runs: i64, me: i64, is_reaction: bool) -> i64 {
  if is_reaction {
    return base_qty * runs;
  }

  let reduced = ((base_qty as f64) * (1.0 - (me as f64) / 100.0)).ceil() as i64;
  reduced.max(1) * runs
}

/// Aggregates a merged build order into the per-type blueprint summary the planner surfaces: one
/// [`NeededBlueprint`] per distinct type with its job count and total runs. Split out so a caller holding an
/// already-computed merged order (the memoized planner plan) can derive blueprints without re-merging.
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

/// Lookups a pure breakdown expansion needs, decoupled from the live planner state so
/// [`expand_to_raw`] can be exercised without any database or UI.
pub trait BuildableLookup<C> {
  /// The buildable input materials of `type_id` (raw, non-producible materials are omitted), in the order
  /// they should be inserted as children.
  fn buildable_inputs(&self, type_id: i64) -> Vec<i64>;

  /// A fresh, un-expanded build config for `type_id` (its own ME/TE/facility defaults, no children).
  fn fresh_child(&self, type_id: i64) -> C;

  /// Mutable access to a child node's own child map, so the expansion can descend.
  fn children_of<'a>(&self, child: &'a mut C) -> &'a mut BTreeMap<i64, C>;
}

/// Recursively breaks down every buildable input of `type_id` across the whole subtree rooted at
/// `children`, down to raw materials. Pure: it touches no database or UI and reads all recipe/buildable
/// facts through `lookup`. Existing children are kept (and themselves expanded) rather than replaced, so an
/// in-progress tree deepens instead of resetting. Manufacturing and reaction nodes are treated alike — both
/// surface buildable inputs through [`BuildableLookup::buildable_inputs`].
pub fn expand_to_raw<C, L>(children: &mut BTreeMap<i64, C>, type_id: i64, lookup: &L)
where
  L: BuildableLookup<C>,
{
  for mat in lookup.buildable_inputs(type_id) {
    let child = children.entry(mat).or_insert_with(|| lookup.fresh_child(mat));
    let grandchildren = lookup.children_of(child);
    expand_to_raw(grandchildren, mat, lookup);
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
        // WIDGET needs 3 COGs; GADGET needs 2 COGs; one root run of each, so 5 COGs total.
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
        // Each parent demands 3 COGs at output_per_run 2: separately ceil(3/2)=2 each (4 runs), but
        // merged ceil(6/2)=3 runs.
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
      fn it_is_unchanged_with_an_empty_allocation() {
        let plan = hulk_plan();

        let netted = plan.raw_totals_after_stock(&StockAllocation::default());

        assert_eq!(netted, plan.raw_totals());
      }

      #[test]
      fn it_subtracts_drawn_stock_from_a_types_demand() {
        // hulk_plan rolls up to 15 Tritanium; drawing 6 from stock leaves 9 to buy.
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
    }
  }

  mod allocate_stock {
    use pretty_assertions::assert_eq;

    use super::*;

    const COMPONENT_A: i64 = 700;
    const SITE_A: i64 = 60_003_760;
    const SITE_B: i64 = 60_008_494;
    const TRITANIUM: i64 = 34;

    #[test]
    fn it_drains_a_shared_pool_in_selection_order() {
      // Example 1: A needs 2000 Trit, B needs 1000, 1000 on hand at the site. "Use Stock" on A first
      // draws all 1000; B then finds the pool empty and must buy its full 1000.
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
    fn it_leaves_the_uncovered_remainder_to_break_down() {
      // Example 2: Fenrir needs 10 of Component A, 5 on hand. The draw is 5 from stock; the remaining 5
      // (the `buy` field) is the uncovered remainder a later breakdown applies to.
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
    fn it_keys_pools_separately_per_site() {
      // Same type at two sites: each draws from its own pool, never the other's.
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
  }

  mod eff_qty {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_ceils_the_me_reduced_quantity() {
      let result = eff_qty(7, 1, 10, false);

      assert_eq!(result, 7);
    }

    #[test]
    fn it_floors_a_reduced_quantity_at_one_per_run() {
      let result = eff_qty(1, 4, 10, false);

      assert_eq!(result, 4);
    }

    #[test]
    fn it_ignores_me_for_reactions() {
      let with_me = eff_qty(100, 3, 10, true);
      let without_me = eff_qty(100, 3, 0, true);

      assert_eq!(with_me, 300);
      assert_eq!(without_me, 300);
    }

    #[test]
    fn it_reduces_manufacturing_quantity_by_me_then_scales_by_runs() {
      let result = eff_qty(100, 3, 10, false);

      assert_eq!(result, 270);
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

  mod expand_to_raw {
    use std::collections::BTreeMap;

    use pretty_assertions::assert_eq;

    use super::*;

    /// A minimal stand-in for the planner's `NodeConfig`: just a child map, so the pure expansion can be
    /// exercised without any UI or planner state.
    #[derive(Clone, Debug, Default, PartialEq)]
    struct TestNode {
      children: BTreeMap<i64, TestNode>,
    }

    /// A buildable lookup backed by a fixed bill-of-materials table keyed by product type id. Any type
    /// absent from the table is treated as a raw material (no buildable inputs).
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

    /// Collects every type id that ends up built in-house across the tree, depth-first.
    fn built_ids(children: &BTreeMap<i64, TestNode>, out: &mut Vec<i64>) {
      for (&id, node) in children {
        out.push(id);
        built_ids(&node.children, out);
      }
    }

    #[test]
    fn it_breaks_down_a_multi_level_tree_to_raw_inputs() {
      // HULK -> RETRIEVER (buildable) + TRITANIUM (raw); RETRIEVER -> TRITANIUM (raw).
      let bom = Bom {
        inputs: BTreeMap::from([(HULK, vec![RETRIEVER]), (RETRIEVER, vec![])]),
      };
      let mut children = BTreeMap::new();

      expand_to_raw(&mut children, HULK, &bom);

      let mut ids = Vec::new();
      built_ids(&children, &mut ids);
      // Only the buildable RETRIEVER becomes a child; raw TRITANIUM is left to buy.
      assert_eq!(ids, vec![RETRIEVER]);
      assert!(children[&RETRIEVER].children.is_empty());
    }

    #[test]
    fn it_descends_through_a_buildable_intermediate() {
      // WIDGET -> GADGET (buildable) -> COG (buildable) -> raw.
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
      // A fuel block reaction whose buildable input is a composite that itself reacts down to raw gas.
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
      // GADGET is already a child but its own COG sub-build is not yet expanded.
      let mut children = BTreeMap::from([(GADGET, TestNode::default())]);

      expand_to_raw(&mut children, WIDGET, &bom);

      // The pre-existing GADGET node is kept and gains its COG child rather than being replaced.
      assert!(children[&GADGET].children.contains_key(&COG));
    }
  }
}
