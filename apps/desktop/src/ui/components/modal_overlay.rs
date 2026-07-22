use iced::{
  Element, Length,
  alignment::{Horizontal, Vertical},
  widget::{Stack, container, opaque},
};

use crate::ui::{components::backdrop, style::spacing};

/// Wraps `card` in a backdrop and a centered scrim layer for use with
/// [`stable_overlay`].
///
/// Only the card is [`opaque`] — the surrounding centering container is not —
/// so a click outside the card falls through to the backdrop button beneath
/// it and dismisses the modal, while a click on the card itself does not.
pub fn modal_layers<'a, M>(dismiss: M, card: Element<'a, M>) -> Vec<Element<'a, M>>
where
  M: Clone + 'a,
{
  vec![backdrop::backdrop(dismiss), modal_scrim(card)]
}

fn modal_scrim<'a, M>(card: Element<'a, M>) -> Element<'a, M>
where
  M: 'a,
{
  container(opaque(card))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(spacing::SPACE_6)
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .into()
}

/// Mounts `base` at child[0] of a [`Stack`] and renders the given overlay
/// `layers` (backdrops, dropdowns, modal content) above it, in order. The
/// root is a `Stack` even when no overlay is active (`layers` is empty), so
/// `base` never changes tree position.
///
/// Iced matches a widget's runtime state (such as a scrollable's internal scroll
/// offset) by tree position + tag. Keeping `base` pinned at child[0] across
/// open/close — only adding/removing sibling layers after it — preserves that
/// state, so an interactive `base` (e.g. a scrolled ledger) does not snap back
/// when a modal opens or closes over it.
pub fn stable_overlay<'a, M>(base: Element<'a, M>, layers: Vec<Element<'a, M>>) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let mut children = vec![base];
  children.extend(layers);

  Stack::with_children(children)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
  use iced::widget::Space;

  use super::*;

  mod stable_overlay {
    use iced::widget::scrollable;

    use super::*;

    fn space() -> Element<'static, ()> {
      Space::new().into()
    }

    fn scroll_base() -> Element<'static, ()> {
      scrollable(space()).into()
    }

    #[test]
    fn it_renders_just_the_base_when_no_layers_are_active() {
      let el = stable_overlay(scroll_base(), vec![]);
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);

      assert_eq!(tree.children.len(), 1);
    }

    #[test]
    fn it_mounts_the_overlay_layers_above_the_base() {
      let el = stable_overlay(scroll_base(), vec![space(), space()]);
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);

      assert_eq!(tree.children.len(), 3);
    }

    #[test]
    fn it_keeps_the_base_at_child_zero_with_a_stable_tag_when_layers_appear() {
      let closed = stable_overlay(scroll_base(), vec![]);
      let mut closed_tree = iced::advanced::widget::Tree::new(&closed);
      closed_tree.diff(&closed);

      let open = stable_overlay(scroll_base(), vec![space(), space()]);
      let mut open_tree = iced::advanced::widget::Tree::new(&open);
      open_tree.diff(&open);

      assert_eq!(closed_tree.children[0].tag, open_tree.children[0].tag);
    }
  }

  mod modal_layers {
    use super::*;

    fn space() -> Element<'static, ()> {
      Space::new().into()
    }

    #[test]
    fn it_produces_a_backdrop_and_a_card_layer() {
      let layers = super::super::modal_layers((), space());

      assert_eq!(layers.len(), 2);
    }

    #[test]
    fn it_mounts_the_backdrop_and_card_over_the_base() {
      let el = stable_overlay(space(), super::super::modal_layers((), space()));
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);

      assert_eq!(tree.children.len(), 3);
    }
  }
}
