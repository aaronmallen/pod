use std::collections::HashMap;

use crate::store::model::{ItemType, MarketGroup};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarketTree {
  pub roots: Vec<MarketNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarketNode {
  pub children: Vec<MarketNode>,
  pub id: i64,
  pub item_count: usize,
  pub items: Vec<MarketLeaf>,
  pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarketLeaf {
  pub best_sell: Option<f64>,
  pub name: String,
  pub type_id: i64,
}

pub fn build_market_tree(groups: &[MarketGroup], items: &[ItemType]) -> MarketTree {
  let mut items_by_group: HashMap<i64, Vec<MarketLeaf>> = HashMap::new();
  for item in items {
    if !item.published() {
      continue;
    }
    let Some(group_id) = item.market_group_id() else {
      continue;
    };
    items_by_group.entry(group_id).or_default().push(MarketLeaf {
      best_sell: None,
      name: item.name().clone(),
      type_id: item.id(),
    });
  }

  let mut children_by_parent: HashMap<Option<i64>, Vec<&MarketGroup>> = HashMap::new();
  for group in groups {
    children_by_parent.entry(group.parent_id()).or_default().push(group);
  }

  MarketTree {
    roots: build_nodes(None, &children_by_parent, &mut items_by_group),
  }
}

#[cfg_attr(
  not(test),
  expect(dead_code, reason = "Consumed by the Phase 3 left-pane filter render task.")
)]
pub fn filter_tree(tree: &MarketTree, query: &str) -> MarketTree {
  let needle = query.trim().to_lowercase();
  if needle.is_empty() {
    return tree.clone();
  }

  MarketTree {
    roots: tree
      .roots
      .iter()
      .filter_map(|node| filter_node(node, &needle))
      .collect(),
  }
}

fn build_nodes(
  parent: Option<i64>,
  children_by_parent: &HashMap<Option<i64>, Vec<&MarketGroup>>,
  items_by_group: &mut HashMap<i64, Vec<MarketLeaf>>,
) -> Vec<MarketNode> {
  let mut nodes: Vec<MarketNode> = children_by_parent
    .get(&parent)
    .map(|groups| {
      groups
        .iter()
        .filter_map(|group| build_node(group, children_by_parent, items_by_group))
        .collect()
    })
    .unwrap_or_default();

  nodes.sort_by(|a, b| a.name.cmp(&b.name));
  nodes
}

fn build_node(
  group: &MarketGroup,
  children_by_parent: &HashMap<Option<i64>, Vec<&MarketGroup>>,
  items_by_group: &mut HashMap<i64, Vec<MarketLeaf>>,
) -> Option<MarketNode> {
  let id = group.id();
  let children = build_nodes(Some(id), children_by_parent, items_by_group);

  let mut items = items_by_group.remove(&id).unwrap_or_default();
  items.sort_by(|a, b| a.name.cmp(&b.name));

  let item_count = items.len() + children.iter().map(|child| child.item_count).sum::<usize>();
  if item_count == 0 {
    return None;
  }

  Some(MarketNode {
    children,
    id,
    item_count,
    items,
    name: group.name().clone(),
  })
}

fn filter_node(node: &MarketNode, needle: &str) -> Option<MarketNode> {
  let group_matches = node.name.to_lowercase().contains(needle);

  let items: Vec<MarketLeaf> = node
    .items
    .iter()
    .filter(|leaf| group_matches || leaf.name.to_lowercase().contains(needle))
    .cloned()
    .collect();

  let children: Vec<MarketNode> = node
    .children
    .iter()
    .filter_map(|child| filter_node(child, needle))
    .collect();

  if items.is_empty() && children.is_empty() {
    return None;
  }

  let item_count = items.len() + children.iter().map(|child| child.item_count).sum::<usize>();

  Some(MarketNode {
    children,
    id: node.id,
    item_count,
    items,
    name: node.name.clone(),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  mod test_support {
    use super::*;

    pub fn mg(id: i64, name: &str, parent: Option<i64>) -> MarketGroup {
      MarketGroup {
        description: String::new(),
        has_types: false,
        icon_id: None,
        id,
        name: name.to_owned(),
        parent_id: parent,
      }
    }

    pub fn it(id: i64, name: &str, market_group_id: Option<i64>) -> ItemType {
      ItemType {
        capacity: None,
        description: None,
        dogma_attributes: "[]".to_owned(),
        group_id: 0,
        icon_id: None,
        id,
        market_group_id,
        name: name.to_owned(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        volume: None,
      }
    }
  }

  mod build_market_tree {
    use pretty_assertions::assert_eq;

    use super::{test_support::*, *};

    fn sample() -> MarketTree {
      let groups = vec![
        mg(1, "Ships", None),
        mg(2, "Frigates", Some(1)),
        mg(3, "Cruisers", Some(1)),
        mg(4, "Materials", None),
      ];
      let items = vec![
        it(587, "Rifter", Some(2)),
        it(588, "Punisher", Some(2)),
        it(621, "Caracal", Some(3)),
        it(34, "Tritanium", Some(4)),
      ];
      build_market_tree(&groups, &items)
    }

    #[test]
    fn it_nests_groups_under_their_parent() {
      let tree = sample();

      // Roots sorted alphabetically: Materials, Ships.
      assert_eq!(tree.roots.len(), 2);
      assert_eq!(tree.roots[0].name, "Materials");
      assert_eq!(tree.roots[1].name, "Ships");

      let ships = &tree.roots[1];
      assert_eq!(ships.children.len(), 2);
      assert_eq!(ships.children[0].name, "Cruisers");
      assert_eq!(ships.children[1].name, "Frigates");
    }

    #[test]
    fn it_attaches_items_to_their_market_group() {
      let tree = sample();
      let frigates = &tree.roots[1].children[1];

      // Leaves sorted alphabetically by name.
      assert_eq!(
        frigates.items.iter().map(|leaf| leaf.type_id).collect::<Vec<_>>(),
        vec![588, 587]
      );
      assert_eq!(frigates.items[0].name, "Punisher");
      assert_eq!(frigates.items[0].best_sell, None);
    }

    #[test]
    fn it_counts_items_recursively() {
      let tree = sample();

      assert_eq!(tree.roots[1].item_count, 3);
      assert_eq!(tree.roots[1].children[1].item_count, 2);
      assert_eq!(tree.roots[0].item_count, 1);
    }

    #[test]
    fn it_ignores_items_without_a_market_group() {
      let groups = vec![mg(1, "Ships", None)];
      let items = vec![it(587, "Rifter", Some(1)), it(999, "Orphan", None)];

      let tree = build_market_tree(&groups, &items);

      assert_eq!(tree.roots[0].item_count, 1);
      assert_eq!(tree.roots[0].items.len(), 1);
    }

    #[test]
    fn it_ignores_unpublished_items() {
      let groups = vec![mg(1, "Ships", None)];
      let mut hidden = it(999, "Test Frigate", Some(1));
      hidden.published = false;

      let tree = build_market_tree(&groups, &[it(587, "Rifter", Some(1)), hidden]);

      assert_eq!(tree.roots[0].item_count, 1);
    }

    #[test]
    fn it_prunes_market_groups_with_no_items() {
      let groups = vec![mg(1, "Ships", None), mg(2, "Empty", None)];
      let items = vec![it(587, "Rifter", Some(1))];

      let tree = build_market_tree(&groups, &items);

      assert_eq!(tree.roots.len(), 1);
      assert_eq!(tree.roots[0].name, "Ships");
    }
  }

  mod filter_tree {
    use pretty_assertions::assert_eq;

    use super::{test_support::*, *};

    fn sample() -> MarketTree {
      let groups = vec![
        mg(1, "Ships", None),
        mg(2, "Frigates", Some(1)),
        mg(3, "Cruisers", Some(1)),
      ];
      let items = vec![
        it(587, "Rifter", Some(2)),
        it(588, "Punisher", Some(2)),
        it(621, "Caracal", Some(3)),
      ];
      build_market_tree(&groups, &items)
    }

    #[test]
    fn it_returns_a_full_clone_for_an_empty_query() {
      let tree = sample();

      assert_eq!(filter_tree(&tree, "   "), tree);
    }

    #[test]
    fn it_keeps_items_matching_by_name() {
      let filtered = filter_tree(&sample(), "rifter");

      assert_eq!(filtered.roots.len(), 1);
      let frigates = &filtered.roots[0].children[0];
      assert_eq!(frigates.name, "Frigates");
      assert_eq!(frigates.items.len(), 1);
      assert_eq!(frigates.items[0].name, "Rifter");
      assert_eq!(filtered.roots[0].item_count, 1);
    }

    #[test]
    fn it_keeps_every_item_when_the_group_name_matches() {
      let filtered = filter_tree(&sample(), "frigate");

      let ships = &filtered.roots[0];
      assert_eq!(ships.children.len(), 1);
      assert_eq!(ships.children[0].name, "Frigates");
      assert_eq!(ships.children[0].items.len(), 2);
    }

    #[test]
    fn it_matches_case_insensitively() {
      let filtered = filter_tree(&sample(), "CARACAL");

      assert_eq!(filtered.roots[0].item_count, 1);
      assert_eq!(filtered.roots[0].children[0].name, "Cruisers");
    }

    #[test]
    fn it_yields_an_empty_tree_when_nothing_matches() {
      let filtered = filter_tree(&sample(), "titan");

      assert!(filtered.roots.is_empty());
    }
  }
}
