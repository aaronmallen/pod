use iced::{
  Element, Length,
  widget::{Column, responsive, scrollable},
};

use super::{AbyssalCard, card, group_by_type};
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

    for (label, members) in group_by_type(cards) {
      // The group starts on a fresh row, so its first visual row is the current
      // slot count divided by the row width.
      group_headers.push((slots.len() / cards_per_row, label));
      for member in &members {
        slots.push(Slot::Card(member));
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
