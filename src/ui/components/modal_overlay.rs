use iced::{Element, Length, widget::Stack};

use crate::ui::components::backdrop;

pub fn modal_overlay<'a, M>(base: Element<'a, M>, backdrop_msg: Option<M>, content: Element<'a, M>) -> Element<'a, M>
where
  M: Clone + 'a,
{
  let mut layers = vec![base];
  if let Some(message) = backdrop_msg {
    layers.push(backdrop::backdrop(message));
  }
  layers.push(content);

  Stack::with_children(layers)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Mounts `base` at child[0] of a [`Stack`] and renders the given overlay
/// `layers` (backdrops, dropdowns, modal content) above it, in order. Unlike
/// [`modal_overlay`], the root is a `Stack` even when no overlay is active
/// (`layers` is empty), so `base` never changes tree position.
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

  mod modal_overlay {
    use super::*;

    fn space() -> Element<'static, ()> {
      Space::new().into()
    }

    #[test]
    fn it_omits_the_backdrop_when_none() {
      let el = modal_overlay(space(), None, space());
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);

      assert_eq!(tree.children.len(), 2);
    }

    #[test]
    fn it_stacks_a_backdrop_between_base_and_content() {
      let el = modal_overlay(space(), Some(()), space());
      let mut tree = iced::advanced::widget::Tree::new(&el);
      tree.diff(&el);

      assert_eq!(tree.children.len(), 3);
    }
  }

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

      // A Stack root with the base alone at child[0], even with no overlay.
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

      // Iced matches runtime state by tree position + tag; child[0] (the base) must
      // carry the same tag whether or not overlay siblings are present, so its
      // state (a scrollable's offset) survives the open/close reshape.
      assert_eq!(closed_tree.children[0].tag, open_tree.children[0].tag);
    }
  }
}
