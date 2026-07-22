use crate::store::model::BudgetOwner;

/// A ledger row, keyed by owner *and* id: under the All-wallets scope a
/// character and a corporation can share an EVE id, so the id alone is ambiguous.
/// Other surfaces (e.g. the asset inventory) key [`RowSelection`] on a different
/// type — the inventory keys on the globally-unique ESI `item_id` (`i64`).
pub type RowKey = (BudgetOwner, i64);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClickKind {
  #[default]
  Plain,
  Range,
  RangeMerge,
  Toggle,
}

impl ClickKind {
  pub fn from_modifiers(command: bool, shift: bool) -> Self {
    match (command, shift) {
      (true, true) => ClickKind::RangeMerge,
      (false, true) => ClickKind::Range,
      (true, false) => ClickKind::Toggle,
      (false, false) => ClickKind::Plain,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowSelection<K = RowKey> {
  anchor: Option<K>,
  selected: Vec<K>,
}

impl<K> Default for RowSelection<K> {
  fn default() -> Self {
    Self {
      anchor: None,
      selected: Vec::new(),
    }
  }
}

impl<K: Copy + PartialEq> RowSelection<K> {
  pub fn contains(&self, key: K) -> bool {
    self.selected.contains(&key)
  }

  pub fn len(&self) -> usize {
    self.selected.len()
  }

  pub fn is_empty(&self) -> bool {
    self.selected.is_empty()
  }

  pub fn clear(&mut self) {
    self.selected.clear();
    self.anchor = None;
  }

  pub fn ordered(&self, order: &[K]) -> Vec<K> {
    order
      .iter()
      .copied()
      .filter(|key| self.selected.contains(key))
      .collect()
  }

  pub fn prune(&mut self, order: &[K]) {
    self.selected.retain(|key| order.contains(key));
    if self.anchor.is_some_and(|a| !order.contains(&a)) {
      self.anchor = None;
    }
  }

  pub fn apply(&mut self, key: K, kind: ClickKind, order: &[K]) {
    match kind {
      ClickKind::Plain => {
        if self.selected.len() == 1 && self.selected[0] == key {
          self.clear();
        } else {
          self.selected = vec![key];
          self.anchor = Some(key);
        }
      }
      ClickKind::Toggle => {
        if let Some(idx) = self.selected.iter().position(|k| *k == key) {
          self.selected.remove(idx);
        } else {
          self.selected.push(key);
        }
        self.anchor = Some(key);
      }
      ClickKind::Range => {
        self.selected = range_keys(self.anchor, key, order);
      }
      ClickKind::RangeMerge => {
        for k in range_keys(self.anchor, key, order) {
          if !self.selected.contains(&k) {
            self.selected.push(k);
          }
        }
      }
    }
  }
}

fn range_keys<K: Copy + PartialEq>(anchor: Option<K>, key: K, order: &[K]) -> Vec<K> {
  let Some(anchor) = anchor else {
    return vec![key];
  };
  let (Some(a), Some(b)) = (
    order.iter().position(|k| *k == anchor),
    order.iter().position(|k| *k == key),
  ) else {
    return vec![key];
  };
  let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
  order[lo..=hi].to_vec()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn char_key(id: i64) -> RowKey {
    (BudgetOwner::Character(1), id)
  }

  fn order() -> Vec<RowKey> {
    vec![char_key(1), char_key(2), char_key(3), char_key(4)]
  }

  mod apply {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_selects_a_single_row_on_a_plain_click() {
      let mut selection = RowSelection::default();

      selection.apply(char_key(2), ClickKind::Plain, &order());

      assert!(selection.contains(char_key(2)));
      assert_eq!(selection.ordered(&order()), vec![char_key(2)]);
    }

    #[test]
    fn it_clears_when_a_plain_click_hits_the_lone_selection() {
      let mut selection = RowSelection::default();
      selection.apply(char_key(2), ClickKind::Plain, &order());

      selection.apply(char_key(2), ClickKind::Plain, &order());

      assert_eq!(selection.len(), 0);
    }

    #[test]
    fn it_toggles_rows_while_keeping_the_rest() {
      let mut selection = RowSelection::default();
      selection.apply(char_key(1), ClickKind::Plain, &order());

      selection.apply(char_key(3), ClickKind::Toggle, &order());

      assert_eq!(selection.ordered(&order()), vec![char_key(1), char_key(3)]);

      selection.apply(char_key(1), ClickKind::Toggle, &order());

      assert_eq!(selection.ordered(&order()), vec![char_key(3)]);
    }

    #[test]
    fn it_selects_a_contiguous_range_from_the_anchor() {
      let mut selection = RowSelection::default();
      selection.apply(char_key(2), ClickKind::Plain, &order());

      selection.apply(char_key(4), ClickKind::Range, &order());

      assert_eq!(selection.ordered(&order()), vec![char_key(2), char_key(3), char_key(4)]);
    }

    #[test]
    fn it_merges_a_range_into_an_existing_selection() {
      let mut selection = RowSelection::default();
      selection.apply(char_key(1), ClickKind::Plain, &order());
      selection.apply(char_key(3), ClickKind::Toggle, &order());

      selection.apply(char_key(4), ClickKind::RangeMerge, &order());

      assert_eq!(selection.ordered(&order()), vec![char_key(1), char_key(3), char_key(4)]);
    }

    #[test]
    fn it_keeps_two_owners_sharing_an_eve_id_distinct() {
      let order = vec![(BudgetOwner::Character(1), 5), (BudgetOwner::Corporation(2), 5)];
      let mut selection = RowSelection::default();

      selection.apply((BudgetOwner::Character(1), 5), ClickKind::Plain, &order);
      selection.apply((BudgetOwner::Corporation(2), 5), ClickKind::Toggle, &order);

      assert_eq!(selection.ordered(&order), order);
    }
  }

  mod generic_key {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_selects_and_prunes_an_i64_keyed_selection() {
      let order = vec![1_i64, 2, 3, 4];
      let mut selection: RowSelection<i64> = RowSelection::default();

      selection.apply(2, ClickKind::Plain, &order);
      selection.apply(4, ClickKind::Range, &order);
      assert_eq!(selection.ordered(&order), vec![2, 3, 4]);

      selection.prune(&[2, 4]);
      assert_eq!(selection.ordered(&[2, 4]), vec![2, 4]);
      assert!(!selection.contains(3));
      assert!(!selection.is_empty());
    }
  }

  mod prune {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_drops_keys_that_left_the_live_order() {
      let mut selection = RowSelection::default();
      selection.apply(char_key(2), ClickKind::Plain, &order());
      selection.apply(char_key(3), ClickKind::Toggle, &order());

      selection.prune(&[char_key(2)]);

      assert_eq!(selection.ordered(&[char_key(2)]), vec![char_key(2)]);
      assert!(!selection.contains(char_key(3)));
    }
  }
}
