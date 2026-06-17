//! A reusable anchored-dropdown widget.
//!
//! Renders an `underlay` (the trigger element) inline. When `open`, it floats a `popover` element in
//! the overlay layer positioned directly below the trigger's layout bounds, width-matched to the
//! trigger, floating *over* the content below without pushing any siblings (no layout shift). An
//! outside click emits `on_dismiss`.
//!
//! This is the same mechanism iced's own `pick_list`/`combo_box` use to anchor their menus: a custom
//! [`Widget`] with a real [`Widget::overlay`] that positions an [`overlay::Element`] from the
//! underlay's bounds. Unlike a cursor-anchored popover, the popover always tracks the trigger and
//! never resizes the surrounding layout.

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

/// Vertical gap between the trigger's bottom edge and the floating popover.
const DROPDOWN_GAP: f32 = 6.0;

/// A widget that anchors a floating `popover` directly below its `underlay` trigger.
pub struct AnchoredDropdown<'a, Message, Theme, Renderer> {
  underlay: Element<'a, Message, Theme, Renderer>,
  popover: Option<Element<'a, Message, Theme, Renderer>>,
  on_dismiss: Option<Message>,
}

impl<'a, Message, Theme, Renderer> AnchoredDropdown<'a, Message, Theme, Renderer>
where
  Renderer: iced::advanced::Renderer,
{
  /// Creates a dropdown around `underlay`. When `popover` is `Some`, it floats below the trigger.
  pub fn new(
    underlay: impl Into<Element<'a, Message, Theme, Renderer>>,
    popover: Option<Element<'a, Message, Theme, Renderer>>,
  ) -> Self {
    Self {
      underlay: underlay.into(),
      popover,
      on_dismiss: None,
    }
  }

  /// Sets the message emitted when the user clicks outside the open popover.
  pub fn on_dismiss(mut self, message: Message) -> Self {
    self.on_dismiss = Some(message);
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
    })))
  }
}

struct DropdownOverlay<'a, 'b, Message, Theme, Renderer> {
  popover: &'a mut Element<'b, Message, Theme, Renderer>,
  tree: &'a mut Tree,
  /// The trigger's bounds in the root coordinate space (anchor for the popover).
  bounds: Rectangle,
  viewport: Rectangle,
  on_dismiss: Option<Message>,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
  for DropdownOverlay<'_, '_, Message, Theme, Renderer>
where
  Message: Clone,
  Renderer: iced::advanced::Renderer,
{
  fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
    // Width-match the trigger; let the popover choose its own height up to the space available.
    let space_below = bounds.height - (self.bounds.y + self.bounds.height + DROPDOWN_GAP);
    let space_above = self.bounds.y - DROPDOWN_GAP;
    let flip_up = space_below < space_above && space_below < self.bounds.height;

    let max_height = if flip_up { space_above } else { space_below }.max(0.0);
    let limits = Limits::new(Size::ZERO, Size::new(self.bounds.width, max_height.max(1.0)))
      .width(Length::Fixed(self.bounds.width));

    let node = self.popover.as_widget_mut().layout(self.tree, renderer, &limits);
    let size = node.size();

    // Keep the popover within the viewport horizontally.
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
    // An outside click (not on the trigger, not on the popover) dismisses the dropdown.
    if let Some(on_dismiss) = &self.on_dismiss
      && let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
    {
      let over_popover = cursor.is_over(layout.bounds());
      let over_trigger = cursor.is_over(self.bounds);
      if !over_popover && !over_trigger {
        shell.publish(on_dismiss.clone());
        shell.capture_event();
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
    // Render above sibling overlays (e.g. the modal panel) so the dropdown floats on top.
    1.0
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
}
