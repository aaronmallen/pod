use iced::{
  Element, Event, Length, Rectangle, Size, Vector,
  advanced::{
    Clipboard, Layout, Shell, Widget,
    layout::{Limits, Node},
    mouse,
    overlay::{self, Element as OverlayElement},
    renderer,
    widget::{Operation, Tree, tree},
  },
};

use crate::ui::components::overlay_layer::OverlayLayer;

const DROPDOWN_GAP: f32 = 6.0;

pub struct AnchoredDropdown<'a, Message, Theme, Renderer> {
  underlay: Element<'a, Message, Theme, Renderer>,
  popover: Option<Element<'a, Message, Theme, Renderer>>,
  on_dismiss: Option<Message>,
  popover_width: Option<f32>,
}

impl<'a, Message, Theme, Renderer> AnchoredDropdown<'a, Message, Theme, Renderer>
where
  Renderer: iced::advanced::Renderer,
{
  pub fn new(
    underlay: impl Into<Element<'a, Message, Theme, Renderer>>,
    popover: Option<Element<'a, Message, Theme, Renderer>>,
  ) -> Self {
    Self {
      underlay: underlay.into(),
      popover,
      on_dismiss: None,
      popover_width: None,
    }
  }

  pub fn on_dismiss(mut self, message: Message) -> Self {
    self.on_dismiss = Some(message);
    self
  }

  pub fn popover_width(mut self, width: f32) -> Self {
    self.popover_width = Some(width);
    self
  }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for AnchoredDropdown<'_, Message, Theme, Renderer>
where
  Message: Clone,
  Renderer: iced::advanced::Renderer,
{
  fn tag(&self) -> tree::Tag {
    tree::Tag::stateless()
  }

  fn state(&self) -> tree::State {
    tree::State::None
  }

  fn children(&self) -> Vec<Tree> {
    let mut children = vec![Tree::new(&self.underlay)];
    children.push(match &self.popover {
      Some(popover) => Tree::new(popover),
      None => Tree::empty(),
    });
    children
  }

  fn diff(&self, tree: &mut Tree) {
    let mut children: Vec<&Element<'_, Message, Theme, Renderer>> = vec![&self.underlay];
    if let Some(popover) = &self.popover {
      children.push(popover);
    }
    tree.diff_children(&children);
  }

  fn size(&self) -> Size<Length> {
    self.underlay.as_widget().size()
  }

  fn size_hint(&self) -> Size<Length> {
    self.underlay.as_widget().size_hint()
  }

  fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
    self
      .underlay
      .as_widget_mut()
      .layout(&mut tree.children[0], renderer, limits)
  }

  fn update(
    &mut self,
    tree: &mut Tree,
    event: &Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, Message>,
    viewport: &Rectangle,
  ) {
    self.underlay.as_widget_mut().update(
      &mut tree.children[0],
      event,
      layout,
      cursor,
      renderer,
      clipboard,
      shell,
      viewport,
    );
  }

  fn draw(
    &self,
    tree: &Tree,
    renderer: &mut Renderer,
    theme: &Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
  ) {
    self
      .underlay
      .as_widget()
      .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
  }

  fn mouse_interaction(
    &self,
    tree: &Tree,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
    renderer: &Renderer,
  ) -> mouse::Interaction {
    self
      .underlay
      .as_widget()
      .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
  }

  fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
    self
      .underlay
      .as_widget_mut()
      .operate(&mut tree.children[0], layout, renderer, operation);
  }

  fn overlay<'b>(
    &'b mut self,
    tree: &'b mut Tree,
    layout: Layout<'b>,
    _renderer: &Renderer,
    viewport: &Rectangle,
    translation: Vector,
  ) -> Option<OverlayElement<'b, Message, Theme, Renderer>> {
    let popover = self.popover.as_mut()?;
    let (_, popover_tree) = tree.children.split_at_mut(1);

    Some(OverlayElement::new(Box::new(DropdownOverlay {
      popover,
      tree: &mut popover_tree[0],
      bounds: layout.bounds() + translation,
      viewport: *viewport,
      on_dismiss: self.on_dismiss.clone(),
      width: self.popover_width,
    })))
  }
}

struct DropdownOverlay<'a, 'b, Message, Theme, Renderer> {
  popover: &'a mut Element<'b, Message, Theme, Renderer>,
  tree: &'a mut Tree,
  bounds: Rectangle,
  viewport: Rectangle,
  on_dismiss: Option<Message>,
  width: Option<f32>,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
  for DropdownOverlay<'_, '_, Message, Theme, Renderer>
where
  Message: Clone,
  Renderer: iced::advanced::Renderer,
{
  fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
    let space_below = bounds.height - (self.bounds.y + self.bounds.height + DROPDOWN_GAP);
    let space_above = self.bounds.y - DROPDOWN_GAP;
    let width = self.width.unwrap_or(self.bounds.width).min(bounds.width).max(1.0);

    // Measure the popover's natural height against the larger available band so the
    // flip decision compares the real popover height to the room below — not the
    // trigger height, which left a tall popover clipped at the bottom of the screen.
    let measure_band = space_below.max(space_above).max(1.0);
    let measure_limits = Limits::new(Size::ZERO, Size::new(width, measure_band)).width(Length::Fixed(width));
    let popover_height = self
      .popover
      .as_widget_mut()
      .layout(self.tree, renderer, &measure_limits)
      .size()
      .height;
    let flip_up = should_flip_up(space_below, space_above, popover_height);

    let max_height = if flip_up { space_above } else { space_below }.max(0.0);
    let limits = Limits::new(Size::ZERO, Size::new(width, max_height.max(1.0))).width(Length::Fixed(width));

    let node = self.popover.as_widget_mut().layout(self.tree, renderer, &limits);
    let size = node.size();

    let max_x = (bounds.width - size.width).max(0.0);
    let x = self.bounds.x.min(max_x).max(0.0);
    let y = if flip_up {
      (self.bounds.y - DROPDOWN_GAP - size.height).max(0.0)
    } else {
      self.bounds.y + self.bounds.height + DROPDOWN_GAP
    };

    node.move_to(iced::Point::new(x, y))
  }

  fn draw(
    &self,
    renderer: &mut Renderer,
    theme: &Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
  ) {
    self
      .popover
      .as_widget()
      .draw(self.tree, renderer, theme, style, layout, cursor, &layout.bounds());
  }

  fn update(
    &mut self,
    event: &Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, Message>,
  ) {
    if let Some(on_dismiss) = &self.on_dismiss
      && let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
    {
      let over_popover = cursor.is_over(layout.bounds());
      let over_trigger = cursor.is_over(self.bounds);
      if !over_popover && !over_trigger {
        // Dismiss without capturing the event: capturing would short-circuit the base-tree pass and
        // swallow the click, so the tab/rail underneath would never receive it. Letting it fall
        // through dismisses the popover and navigates in a single click. The early return still keeps
        // the click out of the popover's own content.
        shell.publish(on_dismiss.clone());
        return;
      }
    }

    let viewport = self.viewport;
    self
      .popover
      .as_widget_mut()
      .update(self.tree, event, layout, cursor, renderer, clipboard, shell, &viewport);
  }

  fn mouse_interaction(&self, layout: Layout<'_>, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
    self
      .popover
      .as_widget()
      .mouse_interaction(self.tree, layout, cursor, &layout.bounds(), renderer)
  }

  fn index(&self) -> f32 {
    OverlayLayer::Dropdown.z()
  }
}

impl<'a, Message, Theme, Renderer> From<AnchoredDropdown<'a, Message, Theme, Renderer>>
  for Element<'a, Message, Theme, Renderer>
where
  Message: Clone + 'a,
  Theme: 'a,
  Renderer: iced::advanced::Renderer + 'a,
{
  fn from(dropdown: AnchoredDropdown<'a, Message, Theme, Renderer>) -> Self {
    Element::new(dropdown)
  }
}

fn should_flip_up(space_below: f32, space_above: f32, popover_height: f32) -> bool {
  space_below < popover_height && space_above > space_below
}

#[cfg(test)]
mod tests {
  use iced::widget::{Space, text};

  use super::*;

  fn underlay() -> Element<'static, (), iced::Theme, iced::Renderer> {
    text("trigger").into()
  }

  mod new {
    use super::*;

    #[test]
    fn it_builds_a_closed_dropdown() {
      let _el: Element<'_, (), iced::Theme, iced::Renderer> = AnchoredDropdown::new(underlay(), None).into();
    }

    #[test]
    fn it_builds_an_open_dropdown_with_a_popover() {
      let popover: Element<'_, (), iced::Theme, iced::Renderer> = Space::new().into();
      let _el: Element<'_, (), iced::Theme, iced::Renderer> =
        AnchoredDropdown::new(underlay(), Some(popover)).on_dismiss(()).into();
    }
  }

  mod index {
    use iced::advanced::overlay::Overlay;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_dropdown_overlay_layer() {
      let mut popover: Element<'_, (), iced::Theme, iced::Renderer> = Space::new().into();
      let mut tree = Tree::empty();
      let dropdown = DropdownOverlay {
        popover: &mut popover,
        tree: &mut tree,
        bounds: Rectangle {
          x: 0.0,
          y: 0.0,
          width: 0.0,
          height: 0.0,
        },
        viewport: Rectangle {
          x: 0.0,
          y: 0.0,
          width: 0.0,
          height: 0.0,
        },
        on_dismiss: None,
        width: None,
      };

      assert_eq!(dropdown.index(), OverlayLayer::Dropdown.z());
    }
  }

  mod should_flip_up {
    use super::*;

    #[test]
    fn it_stays_down_when_the_popover_fits_below() {
      assert!(!should_flip_up(300.0, 100.0, 240.0));
    }

    #[test]
    fn it_flips_up_near_the_bottom_when_more_room_is_above() {
      assert!(should_flip_up(35.0, 500.0, 240.0));
    }

    #[test]
    fn it_stays_down_when_below_is_tight_but_still_the_larger_band() {
      assert!(!should_flip_up(120.0, 80.0, 240.0));
    }
  }
}
