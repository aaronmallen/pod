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
}
