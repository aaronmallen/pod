use std::collections::BTreeMap;

/// ESI activity id for reactions in pod (the design reference used a synthetic 9). Reaction nodes ignore ME.
pub const REACTION_ACTIVITY_ID: i64 = 11;

#[derive(Clone, Debug, PartialEq)]
pub struct BuildJob {
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

  fn order_into(&self, runs: i64, path: &[i64], out: &mut Vec<BuildJob>) {
    for (&mat, child) in &self.children {
      let needed = self.needed_for(mat, runs);
      let child_runs = child.runs_for(needed);
      let mut child_path = path.to_vec();
      child_path.push(mat);

      child.order_into(child_runs, &child_path, out);
    }

    out.push(BuildJob {
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
    self.root.order_into(self.runs, &[], &mut out);
    out
  }

  pub fn collect_builds(&self) -> Vec<SubBuild> {
    let mut out = Vec::new();
    self.root.collect_into(self.runs, &[], 0, &mut out);
    out
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawTotal {
  pub qty: i64,
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

/// Effective material need: `ceil(base * (1 - me/100))` floored at 1 per run, then scaled by runs. Reactions
/// bypass ME entirely (`base * runs`).
pub fn eff_qty(base_qty: i64, runs: i64, me: i64, is_reaction: bool) -> i64 {
  if is_reaction {
    return base_qty * runs;
  }

  let reduced = ((base_qty as f64) * (1.0 - (me as f64) / 100.0)).ceil() as i64;
  reduced.max(1) * runs
}

pub fn runs_for(needed_qty: i64, output_per_run: i64) -> i64 {
  let per_run = output_per_run.max(1);
  let demand = needed_qty.max(0);
  ((demand + per_run - 1) / per_run).max(1)
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
}
