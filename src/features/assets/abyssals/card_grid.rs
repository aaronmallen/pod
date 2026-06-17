use std::{
  cell::RefCell,
  hash::{Hash, Hasher},
};

use iced::{
  Element, Length,
  widget::{Column, responsive, scrollable},
};

use super::{AbyssalCard, card};
use crate::{
  features::assets::Message,
  ui::{
    components::{
      section_header::section_header,
      virtual_list::{VirtualList, VirtualListConfig},
    },
    style::spacing,
  },
};

/// Rendered width of a single abyssal card.
const CARD_WIDTH: f32 = card::CARD_WIDTH;
/// Horizontal gap between cards in a row (mirrors the wrapped row's spacing).
const CARD_GAP: f32 = spacing::SPACE_3_5;

/// Nominal height of one row of cards, in pixels.
///
/// Abyssal cards are content-driven (a variable number of rolled-stat rows plus an
/// optional section header on a group's first row), so this is only an estimate
/// for [`VirtualList`] offset math; overscan absorbs the variance.
const ESTIMATED_ROW_HEIGHT: f32 = 300.0;

thread_local! {
  /// Last computed grouping plan, keyed by [`card_set_fingerprint`].
  ///
  /// `view()` re-runs every iced frame, so the plan is memoized here to avoid re-grouping
  /// on every render. It stores owned *indices* (not card refs) precisely so it can outlive
  /// any one frame's borrow and be rehydrated against the current card slice each render.
  static GROUP_PLAN: RefCell<Option<(u64, GroupPlan)>> = const { RefCell::new(None) };
}

type GroupPlan = Vec<(String, Vec<usize>)>;

/// A grid flattened into a single row-major index space, plus the metadata the row
/// renderer needs to reattach section headers.
struct GridLayout<'a> {
  cards_per_row: usize,
  /// `(first_row_index, label)` for each module-type group, in display order.
  group_headers: Vec<(usize, String)>,
  /// Card/pad slots, grouped `cards_per_row` to a visual row.
  slots: Vec<Slot<'a>>,
}

impl<'a> GridLayout<'a> {
  /// Flatten the grouped cards into a padded, row-major slot list.
  fn build(cards: &[&'a AbyssalCard], cards_per_row: usize) -> Self {
    let cards_per_row = cards_per_row.max(1);
    let mut slots: Vec<Slot<'a>> = Vec::with_capacity(cards.len());
    let mut group_headers: Vec<(usize, String)> = Vec::new();

    for (label, members) in grouped_indices(cards) {
      // The group starts on a fresh row, so its first visual row is the current
      // slot count divided by the row width.
      group_headers.push((slots.len() / cards_per_row, label));
      for index in &members {
        slots.push(Slot::Card(cards[*index]));
      }
      // Pad the group's final row so the next group begins on its own row.
      let remainder = slots.len() % cards_per_row;
      if remainder != 0 {
        for _ in remainder..cards_per_row {
          slots.push(Slot::Pad);
        }
      }
    }

    Self {
      cards_per_row,
      group_headers,
      slots,
    }
  }

  /// The header label, if any group begins on this visual row.
  fn header_for_row(&self, row: usize) -> Option<&str> {
    self
      .group_headers
      .iter()
      .find(|(first_row, _)| *first_row == row)
      .map(|(_, label)| label.as_str())
  }

  /// Build one visual row: its (optional) section header above its cards.
  fn render_row(&self, row: usize) -> Element<'a, Message> {
    let start = row * self.cards_per_row;
    let end = (start + self.cards_per_row).min(self.slots.len());

    let cells: Vec<Element<'a, Message>> = self.slots[start..end]
      .iter()
      .filter_map(|slot| match slot {
        Slot::Card(card_data) => Some(card::view(card_data)),
        Slot::Pad => None,
      })
      .collect();
    let cards_row = iced::widget::Row::with_children(cells).spacing(CARD_GAP);

    let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(2);
    if let Some(label) = self.header_for_row(row) {
      let count = group_card_count(&self.group_headers, &self.slots, row, self.cards_per_row);
      children.push(section_header(label, Some(&count)));
    }
    children.push(cards_row.into());

    Column::with_children(children)
      .spacing(spacing::SPACE_3)
      .width(Length::Fill)
      .into()
  }

  #[cfg(test)]
  fn total_rows(&self) -> usize {
    self.slots.len().div_ceil(self.cards_per_row)
  }
}

/// One position in the flattened grid.
///
/// Groups are padded out to a whole row at their boundary so a row never straddles
/// two module types; [`Slot::Pad`] marks those trailing fillers.
enum Slot<'a> {
  Card(&'a AbyssalCard),
  Pad,
}

/// Render the card grid windowed to the viewport.
///
/// The grid is flattened into a row-major slot space (grouped by module type, with
/// section headers reattached to each group's first row) and windowed by
/// [`VirtualList`] so only the viewport's card-rows are materialized regardless of
/// how many pages have been loaded.
pub(super) fn windowed_grid<'a>(cards: Vec<&'a AbyssalCard>, scroll_offset: f32) -> Element<'a, Message> {
  responsive(move |size| {
    let per_row = cards_per_row(size.width);
    let layout = GridLayout::build(&cards, per_row);
    let config = VirtualListConfig::new(layout.slots.len(), ESTIMATED_ROW_HEIGHT)
      .items_per_row(layout.cards_per_row)
      .viewport_height(size.height)
      .scroll_offset(scroll_offset);

    let grid = VirtualList::new(config, move |row| layout.render_row(row))
      .spacing(spacing::SPACE_6)
      .view();

    scrollable(grid)
      .style(crate::ui::style::control::scrollbar)
      .width(Length::Fill)
      .height(Length::Fill)
      .on_scroll(|viewport| Message::AbyssalGridScrolled {
        absolute: viewport.absolute_offset().y,
        relative: viewport.relative_offset().y,
      })
      .into()
  })
  .into()
}

/// Hashes only the grouping inputs (`group_type_id` + `module_name`, in order).
///
/// `item_id`, prices, and rolled stats are deliberately excluded: they change without
/// affecting the grouping, so changing them must not invalidate the cached plan.
fn card_set_fingerprint(cards: &[&AbyssalCard]) -> u64 {
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  cards.len().hash(&mut hasher);
  for card in cards {
    card.group_type_id.hash(&mut hasher);
    card.module_name.hash(&mut hasher);
  }
  hasher.finish()
}

/// Number of cards that fit across the available width.
fn cards_per_row(width: f32) -> usize {
  if width < CARD_WIDTH {
    return 1;
  }
  // n cards take n*CARD_WIDTH + (n-1)*CARD_GAP; solve for the largest n that fits.
  (((width + CARD_GAP) / (CARD_WIDTH + CARD_GAP)).floor() as usize).max(1)
}

/// Count the real (non-pad) cards in the group that begins at `row`.
fn group_card_count(group_headers: &[(usize, String)], slots: &[Slot<'_>], row: usize, cards_per_row: usize) -> String {
  let start = row * cards_per_row;
  // The group runs until the next group's first row (or the end of the slots).
  let end_row = group_headers
    .iter()
    .map(|(first_row, _)| *first_row)
    .filter(|first_row| *first_row > row)
    .min()
    .map_or(slots.len(), |next_first_row| next_first_row * cards_per_row);

  let count = slots[start..end_row.min(slots.len())]
    .iter()
    .filter(|slot| matches!(slot, Slot::Card(_)))
    .count();
  format!("{count} module{}", if count == 1 { "" } else { "s" })
}

fn group_indices_by_type(cards: &[&AbyssalCard]) -> GroupPlan {
  let mut order: Vec<i64> = Vec::new();
  let mut groups: std::collections::HashMap<i64, (String, Vec<usize>)> = std::collections::HashMap::new();
  for (index, card) in cards.iter().enumerate() {
    groups
      .entry(card.group_type_id)
      .or_insert_with(|| {
        order.push(card.group_type_id);
        (card.module_name.clone(), Vec::new())
      })
      .1
      .push(index);
  }
  order
    .into_iter()
    .map(|type_id| {
      groups
        .remove(&type_id)
        .unwrap_or_else(|| (format!("Type {type_id}"), Vec::new()))
    })
    .collect()
}

fn grouped_indices(cards: &[&AbyssalCard]) -> GroupPlan {
  let fingerprint = card_set_fingerprint(cards);

  GROUP_PLAN.with_borrow_mut(|cache| {
    if let Some((cached_fingerprint, plan)) = cache
      && *cached_fingerprint == fingerprint
    {
      return plan.clone();
    }

    let plan = group_indices_by_type(cards);
    *cache = Some((fingerprint, plan.clone()));
    plan
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{features::assets::abyssals::AbyssalStat, store::images};

  fn card(item_id: i64, module: &str, group_type_id: i64) -> AbyssalCard {
    AbyssalCard {
      character_id: 7,
      estimate: Some(1_000_000.0),
      group_type_id,
      item_id,
      location: "Jita IV - Moon 4".to_owned(),
      module_name: module.to_owned(),
      owner_name: "Vex".to_owned(),
      portrait: images::ImageState::Stale {
        id: 7,
        kind: images::ImageKind::CharacterPortrait,
      },
      price_unavailable: false,
      stats: vec![AbyssalStat {
        attribute_id: 50,
        base_value: 47.0,
        bound_hi: 56.0,
        bound_lo: 28.0,
        display_name: "Stasis".to_owned(),
        high_is_good: true,
        rolled: 41.0,
        unit_suffix: " tf".to_owned(),
      }],
      tier_label: "Gravid".to_owned(),
    }
  }

  mod card_set_fingerprint {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_changes_when_the_grouping_inputs_change_and_holds_when_they_do_not() {
      let base = [card(1, "Launcher", 2410), card(2, "Field", 2281)];
      let base_refs: Vec<&AbyssalCard> = base.iter().collect();

      let same = [card(9, "Launcher", 2410), card(8, "Field", 2281)];
      let same_refs: Vec<&AbyssalCard> = same.iter().collect();
      let reordered_refs: Vec<&AbyssalCard> = base.iter().rev().collect();
      let added = [
        card(1, "Launcher", 2410),
        card(2, "Field", 2281),
        card(3, "Field", 2281),
      ];
      let added_refs: Vec<&AbyssalCard> = added.iter().collect();

      // item_id is not a grouping input, so changing only it leaves the fingerprint stable.
      assert_eq!(card_set_fingerprint(&base_refs), card_set_fingerprint(&same_refs));
      assert_ne!(card_set_fingerprint(&base_refs), card_set_fingerprint(&reordered_refs));
      assert_ne!(card_set_fingerprint(&base_refs), card_set_fingerprint(&added_refs));
    }
  }

  mod cards_per_row {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_never_returns_fewer_than_one() {
      assert_eq!(cards_per_row(0.0), 1);
      assert_eq!(cards_per_row(CARD_WIDTH - 1.0), 1);
    }

    #[test]
    fn it_fits_as_many_whole_cards_plus_gaps_as_the_width_allows() {
      assert_eq!(cards_per_row(CARD_WIDTH), 1);
      assert_eq!(cards_per_row(CARD_WIDTH * 2.0 + CARD_GAP), 2);
      assert_eq!(cards_per_row(CARD_WIDTH * 3.0 + CARD_GAP * 2.0), 3);
      // A hair under three cards' worth still only fits two.
      assert_eq!(cards_per_row(CARD_WIDTH * 3.0 + CARD_GAP * 2.0 - 1.0), 2);
    }
  }

  mod grid_layout {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pads_each_group_to_a_whole_row_so_groups_never_share_a_row() {
      let cards = [
        card(1, "Launcher", 2410),
        card(2, "Launcher", 2410),
        card(3, "Launcher", 2410),
        card(4, "Field", 2281),
      ];
      let refs: Vec<&AbyssalCard> = cards.iter().collect();

      let layout = GridLayout::build(&refs, 2);

      // Group A (3 cards) -> 2 rows (one trailing pad); group B (1 card) -> 1 row.
      assert_eq!(layout.total_rows(), 3);
      assert_eq!(layout.slots.len(), 6);
      assert!(matches!(layout.slots[3], Slot::Pad));
      assert_eq!(layout.group_headers[0], (0, "Launcher".to_owned()));
      // Group B starts on its own fresh row, never sharing group A's padded row.
      assert_eq!(layout.group_headers[1], (2, "Field".to_owned()));
    }

    #[test]
    fn it_attaches_a_header_only_to_each_groups_first_row() {
      let cards = [
        card(1, "Launcher", 2410),
        card(2, "Launcher", 2410),
        card(3, "Field", 2281),
      ];
      let refs: Vec<&AbyssalCard> = cards.iter().collect();

      let layout = GridLayout::build(&refs, 1);

      assert_eq!(layout.header_for_row(0), Some("Launcher"));
      assert_eq!(layout.header_for_row(1), None);
      assert_eq!(layout.header_for_row(2), Some("Field"));
    }

    #[test]
    fn it_counts_only_real_cards_in_a_groups_header() {
      let cards = [
        card(1, "Launcher", 2410),
        card(2, "Launcher", 2410),
        card(3, "Launcher", 2410),
        card(4, "Field", 2281),
      ];
      let refs: Vec<&AbyssalCard> = cards.iter().collect();
      let layout = GridLayout::build(&refs, 2);

      // Group A has 3 real cards even though its rows include a pad slot.
      assert_eq!(
        group_card_count(&layout.group_headers, &layout.slots, 0, layout.cards_per_row),
        "3 modules"
      );
      assert_eq!(
        group_card_count(&layout.group_headers, &layout.slots, 2, layout.cards_per_row),
        "1 module"
      );
    }

    #[test]
    fn it_renders_a_windowed_row() {
      let cards = [card(1, "Launcher", 2410), card(2, "Launcher", 2410)];
      let refs: Vec<&AbyssalCard> = cards.iter().collect();
      let layout = GridLayout::build(&refs, 2);

      let _el: Element<'_, Message> = layout.render_row(0);
    }
  }

  mod group_indices_by_type {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_groups_indices_in_first_seen_order_with_each_groups_label() {
      let cards = [
        card(1, "Launcher", 2410),
        card(2, "Field", 2281),
        card(3, "Launcher", 2410),
      ];
      let refs: Vec<&AbyssalCard> = cards.iter().collect();

      let plan = group_indices_by_type(&refs);

      assert_eq!(plan.len(), 2);
      assert_eq!(plan[0], ("Launcher".to_owned(), vec![0, 2]));
      assert_eq!(plan[1], ("Field".to_owned(), vec![1]));
    }
  }

  mod grouped_indices {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_same_grouping_whether_or_not_the_plan_is_cached() {
      let cards = [
        card(1, "Launcher", 2410),
        card(2, "Field", 2281),
        card(3, "Launcher", 2410),
      ];
      let refs: Vec<&AbyssalCard> = cards.iter().collect();

      let first = grouped_indices(&refs);
      let cached = grouped_indices(&refs);

      assert_eq!(first, cached);
      assert_eq!(first, group_indices_by_type(&refs));
    }

    #[test]
    fn it_rebuilds_the_plan_when_the_card_set_changes() {
      let before = [card(1, "Launcher", 2410)];
      let before_refs: Vec<&AbyssalCard> = before.iter().collect();
      let _ = grouped_indices(&before_refs);

      let after = [card(1, "Launcher", 2410), card(2, "Field", 2281)];
      let after_refs: Vec<&AbyssalCard> = after.iter().collect();
      let plan = grouped_indices(&after_refs);

      assert_eq!(plan, group_indices_by_type(&after_refs));
      assert_eq!(plan.len(), 2);
    }
  }

  mod windowed_grid {
    use super::*;

    #[test]
    fn it_builds_the_windowed_grid_element() {
      let cards = [card(1, "Launcher", 2410), card(2, "Field", 2281)];
      let refs: Vec<&AbyssalCard> = cards.iter().collect();

      let _el: Element<'_, Message> = windowed_grid(refs, 0.0);
    }
  }
}
